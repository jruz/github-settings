use std::env;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("GITHUB_TOKEN is not set")]
    #[diagnostic(help("export a token with repo scope: `export GITHUB_TOKEN=$(gh auth token)`"))]
    MissingToken,
}

pub fn token() -> Result<String, Error> {
    let value = env::var("GITHUB_TOKEN");
    validate(value)
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
}
