use std::time;

use reqwest::header;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("{0}")]
    #[diagnostic(help("GITHUB_TOKEN needs admin access to this repository"))]
    Forbidden(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    #[diagnostic(help("GitHub rejected a value; check this key against the schema"))]
    Invalid(String),
    #[error("GITHUB_TOKEN contains invalid HTTP header characters")]
    InvalidToken,
    #[error("{0}")]
    #[diagnostic(help("the repository does not exist or GITHUB_TOKEN cannot see it"))]
    NotFound(String),
    #[error("{0}")]
    Rejected(String),
    #[error("{0}")]
    #[diagnostic(help(
        "GITHUB_TOKEN is invalid or expired; run `export GITHUB_TOKEN=$(gh auth token)`"
    ))]
    Unauthorized(String),
}

pub struct Client {
    pub base: String,
    pub http: reqwest::Client,
}

pub fn client(token: &str, base: &str) -> Result<Client, Error> {
    let bearer = format!("Bearer {token}");
    let authorization = header::HeaderValue::from_str(&bearer);
    let Ok(mut authorization) = authorization else {
        return Err(Error::InvalidToken);
    };
    authorization.set_sensitive(true);
    let mut headers = header::HeaderMap::new();
    headers.insert(header::AUTHORIZATION, authorization);
    let accept = header::HeaderValue::from_static("application/vnd.github+json");
    headers.insert(header::ACCEPT, accept);
    let version = header::HeaderValue::from_static("2022-11-28");
    headers.insert("X-GitHub-Api-Version", version);
    let policy = reqwest::redirect::Policy::none();
    let timeout = time::Duration::from_secs(30);
    let http = reqwest::Client::builder()
        .default_headers(headers)
        .user_agent(concat!("github-settings/", env!("CARGO_PKG_VERSION")))
        .redirect(policy)
        .timeout(timeout)
        .build()?;
    Ok(Client {
        http,
        base: base.to_owned(),
    })
}

#[tracing::instrument(skip(client, payload))]
pub async fn request(
    client: &Client,
    method: reqwest::Method,
    path: &str,
    payload: Option<&serde_json::Value>,
) -> Result<reqwest::Response, Error> {
    let url = format!("{}{path}", client.base);
    let request = client.http.request(method, url);
    let request = if let Some(payload) = payload {
        request.json(payload)
    } else {
        request
    };
    let response = request.send().await?;
    Ok(response)
}

pub async fn successful(
    response: reqwest::Response,
    method: reqwest::Method,
    path: &str,
) -> Result<reqwest::Response, Error> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await?;
        let error = rejected(&method, path, status, &body);
        return Err(error);
    }
    Ok(response)
}

pub fn rejected(
    method: &reqwest::Method,
    path: &str,
    status: reqwest::StatusCode,
    body: &str,
) -> Error {
    tracing::error!(
        %method,
        path,
        status = status.as_u16(),
        body,
        "GitHub rejected request"
    );
    let message = message(status, body);
    match status {
        reqwest::StatusCode::UNAUTHORIZED => Error::Unauthorized(message),
        reqwest::StatusCode::FORBIDDEN => Error::Forbidden(message),
        reqwest::StatusCode::NOT_FOUND => Error::NotFound(message),
        reqwest::StatusCode::UNPROCESSABLE_ENTITY => Error::Invalid(message),
        _ => Error::Rejected(message),
    }
}

fn message(status: reqwest::StatusCode, body: &str) -> String {
    let value = serde_json::from_str::<serde_json::Value>(body);
    let message = value
        .as_ref()
        .map(|value| value.get("message").and_then(serde_json::Value::as_str));
    match (message, status.canonical_reason()) {
        (Ok(Some(message)), _) => message.to_owned(),
        (Ok(None) | Err(_), Some(reason)) => reason.to_owned(),
        (Ok(None) | Err(_), None) => status.as_str().to_owned(),
    }
}

pub fn segment(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC).to_string()
}
