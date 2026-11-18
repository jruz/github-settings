use std::env;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("GITHUB_API_URL is not valid UTF-8")]
    #[diagnostic(help("unset GITHUB_API_URL to use https://api.github.com/"))]
    InvalidApiUrl,
    #[error("GITHUB_TOKEN is not set")]
    #[diagnostic(help("export a token with repo scope: `export GITHUB_TOKEN=$(gh auth token)`"))]
    MissingToken,
}

pub fn token() -> Result<String, Error> {
    let value = env::var("GITHUB_TOKEN");
    validate(value)
}

pub fn api_url() -> Result<String, Error> {
    let Some(value) = env::var_os("GITHUB_API_URL") else {
        let github = "https://api.github.com/".to_owned();
        return Ok(github);
    };
    let Ok(url) = value.into_string() else {
        return Err(Error::InvalidApiUrl);
    };
    let base = slashed(url);
    Ok(base)
}

fn slashed(url: String) -> String {
    if url.ends_with('/') {
        return url;
    }
    format!("{url}/")
}

fn validate(value: Result<String, env::VarError>) -> Result<String, Error> {
    match value {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        Ok(_) | Err(_) => Err(Error::MissingToken),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn token_is_required_without_disclosing_it() {
        assert_eq!(
            super::validate(Err(std::env::VarError::NotPresent))
                .expect_err("missing token")
                .to_string(),
            "GITHUB_TOKEN is not set"
        );
        let empty = String::new();
        let empty = Ok(empty);
        super::validate(empty).expect_err("empty token rejected");
        let blank = " \n".to_owned();
        let blank = Ok(blank);
        super::validate(blank).expect_err("blank token rejected");
        let token = "test-token".to_owned();
        let token = Ok(token);
        assert_eq!(super::validate(token).expect("token"), "test-token");
    }

    #[test]
    fn api_url_ends_with_a_slash() {
        let expected = "http://127.0.0.1:8080/";
        for url in ["http://127.0.0.1:8080", expected] {
            let url = url.to_owned();
            assert_eq!(super::slashed(url), expected);
        }
    }
}
