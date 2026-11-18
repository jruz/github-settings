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
    let settings: types::Settings = read_json(client, &base).await?;
    let branch = match branch {
        Some(branch) => branch,
        None => &settings.default_branch,
    };
    let path = format!("{base}/branches/{}/protection", api::segment(branch));
    let branch_protection = read_protection(client, &path, settings.private).await?;
    let declaration = types::Declaration {
        repository: repository.clone(),
        settings,
        topics: read_json(client, &format!("{base}/topics")).await?,
        branch_protection,
        actions: read_json(client, &format!("{base}/actions/permissions")).await?,
        security: types::Security {
            vulnerability_alerts: read_toggle(client, &format!("{base}/vulnerability-alerts"))
                .await?,
            automated_security_fixes: read_fixes(
                client,
                &format!("{base}/automated-security-fixes"),
            )
            .await?,
        },
        labels: read_labels(client, &base).await?,
    };
    let declaration = types::normalize(declaration)?;
    Ok(declaration)
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
        let error = api::Error::Status {
            body,
            status,
            method: reqwest::Method::GET,
            path: path.to_owned(),
        };
        let error = Error::Api(error);
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
