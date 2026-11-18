use crate::utils::types;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("declaration repository {declared} does not match Git origin {origin}")]
    Mismatch { declared: String, origin: String },
    #[error("could not read Git origin: {0}")]
    Origin(String),
    #[error("Git origin must be a GitHub HTTPS or SSH URL")]
    Remote,
    #[error(transparent)]
    #[diagnostic(transparent)]
    Repository(#[from] types::Error),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
}

#[tracing::instrument]
pub async fn repository() -> Result<types::Repository, Error> {
    let output = tokio::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .await?;
    if !output.status.success() {
        let stderr = String::from_utf8(output.stderr)?;
        let message = stderr.trim().to_owned();
        let error = Error::Origin(message);
        return Err(error);
    }
    let remote = String::from_utf8(output.stdout)?;
    let remote = remote.trim();
    parse_remote(remote)
}

fn parse_remote(remote: &str) -> Result<types::Repository, Error> {
    let path = [
        "https://github.com/",
        "git@github.com:",
        "ssh://git@github.com/",
    ]
    .iter()
    .find_map(|prefix| remote.strip_prefix(prefix))
    .ok_or(Error::Remote)?;
    let path = match path.strip_suffix(".git") {
        Some(path) => path,
        None => path,
    };
    let repository = types::repository(path)?;
    Ok(repository)
}

pub fn check_repository(
    declared: &types::Repository,
    origin: &types::Repository,
) -> Result<(), Error> {
    if !declared.0.eq_ignore_ascii_case(&origin.0) {
        return Err(Error::Mismatch {
            declared: declared.0.clone(),
            origin: origin.0.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn github_remotes_resolve_to_repository() {
        for remote in [
            "https://github.com/owner/repo.git",
            "https://github.com/owner/repo",
            "git@github.com:owner/repo.git",
            "ssh://git@github.com/owner/repo.git",
        ] {
            let repository = super::parse_remote(remote).expect("GitHub remote");
            assert_eq!(repository.0, "owner/repo");
        }
    }

    #[test]
    fn unsupported_or_invalid_remotes_fail() {
        for remote in [
            "https://example.com/owner/repo.git",
            "https://github.com/owner",
            "https://github.com/owner/repo/extra",
            "/local/repo",
        ] {
            super::parse_remote(remote).expect_err("invalid remote");
        }
    }

    #[test]
    fn declarations_cannot_target_another_repository() {
        let origin = super::types::repository("owner/repo").expect("origin");
        let matching = super::types::repository("Owner/Repo").expect("matching repository");
        let other = super::types::repository("owner/other").expect("other repository");
        super::check_repository(&matching, &origin).expect("same repository");
        super::check_repository(&other, &origin).expect_err("different repository");
    }
}
