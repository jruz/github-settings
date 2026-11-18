use super::{api, protection};
use crate::utils::types;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Api(#[from] api::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Protection(#[from] protection::Error),
    #[error("could not read {0} for {1}")]
    Read(
        String,
        String,
        #[source]
        #[diagnostic_source]
        Box<dyn miette::Diagnostic + Send + Sync>,
    ),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Validation(#[from] types::Error),
}

#[tracing::instrument(skip(client))]
pub async fn read(
    client: &api::Client,
    repository: &types::Repository,
    branch: Option<&str>,
) -> Result<types::Declaration, Error> {
    let base = format!("repos/{}", repository.0);
    let settings_read = read_json(client, &base);
    let settings: types::Settings = within("settings", repository, settings_read).await?;
    let branch = match branch {
        Some(branch) => branch,
        None => &settings.default_branch,
    };
    let protection_path = format!("{base}/branches/{}/protection", api::segment(branch));
    let protection_read = read_protection(client, &protection_path, settings.private);
    let branch_protection = within("branch_protection", repository, protection_read).await?;
    let topics_path = format!("{base}/topics");
    let topics_read = read_json(client, &topics_path);
    let topics = within("topics", repository, topics_read).await?;
    let actions_path = format!("{base}/actions/permissions");
    let actions_read = read_json(client, &actions_path);
    let actions = within("actions", repository, actions_read).await?;
    let alerts_path = format!("{base}/vulnerability-alerts");
    let alerts_read = read_toggle(client, &alerts_path);
    let vulnerability_alerts =
        within("security.vulnerability_alerts", repository, alerts_read).await?;
    let fixes_path = format!("{base}/automated-security-fixes");
    let fixes_read = read_fixes(client, &fixes_path);
    let automated_security_fixes =
        within("security.automated_security_fixes", repository, fixes_read).await?;
    let labels_read = read_labels(client, &base);
    let labels = within("labels", repository, labels_read).await?;
    let declaration = types::Declaration {
        repository: repository.clone(),
        settings,
        topics,
        branch_protection,
        actions,
        security: types::Security {
            automated_security_fixes,
            vulnerability_alerts,
        },
        labels,
    };
    let declaration = types::normalize(declaration)?;
    Ok(declaration)
}

async fn within<T>(
    key: &str,
    repository: &types::Repository,
    read: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    match read.await {
        Ok(value) => Ok(value),
        Err(source) => {
            let source = Box::new(source);
            let key = key.to_owned();
            let repository = repository.0.clone();
            let error = Error::Read(key, repository, source);
            Err(error)
        }
    }
}

async fn read_protection(
    client: &api::Client,
    path: &str,
    private: bool,
) -> Result<Option<types::Protection>, Error> {
    let response = api::request(client, reqwest::Method::GET, path, None).await?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if private && status == reqwest::StatusCode::FORBIDDEN {
        let body = response.text().await?;
        let value: serde_json::Value = serde_json::from_str(&body)?;
        let message = value.get("message").and_then(serde_json::Value::as_str);
        let restricted =
            "Upgrade to GitHub Pro or make this repository public to enable this feature.";
        if message == Some(restricted) {
            return Ok(None);
        }
        let rejection = api::rejected(&reqwest::Method::GET, path, status, &body);
        let error = Error::Api(rejection);
        return Err(error);
    }
    let response = api::successful(response, reqwest::Method::GET, path).await?;
    let value = response.json().await?;
    let protection = protection::normalize(value)?;
    let protection = Some(protection);
    Ok(protection)
}

async fn read_json<T>(client: &api::Client, path: &str) -> Result<T, Error>
where
    T: serde::de::DeserializeOwned,
{
    let response = api::request(client, reqwest::Method::GET, path, None).await?;
    let response = api::successful(response, reqwest::Method::GET, path).await?;
    let value = response.json().await?;
    Ok(value)
}

async fn read_toggle(client: &api::Client, path: &str) -> Result<bool, Error> {
    let response = api::request(client, reqwest::Method::GET, path, None).await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(false);
    }
    api::successful(response, reqwest::Method::GET, path).await?;
    Ok(true)
}

async fn read_fixes(client: &api::Client, path: &str) -> Result<bool, Error> {
    let response = api::request(client, reqwest::Method::GET, path, None).await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(false);
    }
    let enabled: types::Enabled = api::successful(response, reqwest::Method::GET, path)
        .await?
        .json()
        .await?;
    Ok(enabled.enabled)
}

async fn read_labels(client: &api::Client, base: &str) -> Result<Vec<types::Label>, Error> {
    let mut labels = Vec::new();
    let mut page: usize = 1;
    loop {
        let batch: Vec<types::Label> =
            read_json(client, &format!("{base}/labels?per_page=100&page={page}")).await?;
        let last = batch.len() < 100;
        labels.extend(batch);
        if last {
            return Ok(labels);
        }
        page += 1;
    }
}
