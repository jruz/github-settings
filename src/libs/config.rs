use std::{io, path};

use tokio::io::AsyncWriteExt as _;

use crate::utils::types;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("declaration already exists: {0}; use --force to overwrite")]
    Exists(path::PathBuf),
    #[error("{file} has keys with \"-\"; rename them to match the schema\n{renames}")]
    Hyphenated { file: String, renames: String },
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("cannot read declaration {0}: {1}")]
    Read(path::PathBuf, io::Error),
    #[error("unknown fields in declaration: {0}")]
    UnknownFields(String),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Validation(#[from] types::Error),
}

#[tracing::instrument]
pub async fn read(file: &path::Path) -> Result<types::Declaration, Error> {
    let content = tokio::fs::read(file).await.map_err(|source| {
        let path = file.to_owned();
        Error::Read(path, source)
    })?;
    let value: serde_json::Value = serde_json::from_slice(&content)?;
    let renames = hyphenated(&value, "");
    if !renames.is_empty() {
        return Err(Error::Hyphenated {
            file: file.display().to_string(),
            renames: renames.join("\n"),
        });
    }
    let declaration = strict(&content)?;
    let declaration = types::normalize(declaration)?;
    Ok(declaration)
}

fn hyphenated(value: &serde_json::Value, parent: &str) -> Vec<String> {
    match value {
        serde_json::Value::Object(fields) => fields
            .iter()
            .flat_map(|(key, child)| {
                let current = join(parent, key);
                let corrected = key.replace('-', "_");
                let target = join(parent, &corrected);
                let rename = key
                    .contains('-')
                    .then(|| format!("  - {current} → {target}"));
                let nested = hyphenated(child, &current);
                rename.into_iter().chain(nested)
            })
            .collect(),
        serde_json::Value::Array(items) => items
            .iter()
            .enumerate()
            .flat_map(|(index, item)| {
                let position = index.to_string();
                let current = join(parent, &position);
                hyphenated(item, &current)
            })
            .collect(),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => Vec::new(),
    }
}

fn join(parent: &str, key: &str) -> String {
    if parent.is_empty() {
        return key.to_owned();
    }
    format!("{parent}.{key}")
}

fn strict(content: &[u8]) -> Result<types::Declaration, Error> {
    let mut unknown = Vec::new();
    let deserializer = &mut serde_json::Deserializer::from_slice(content);
    let declaration = serde_ignored::deserialize(deserializer, |field| {
        let field = field.to_string();
        unknown.push(field);
    })?;
    if !unknown.is_empty() {
        let fields = unknown.join(", ");
        let error = Error::UnknownFields(fields);
        return Err(error);
    }
    Ok(declaration)
}

#[tracing::instrument]
pub async fn check_destination(file: &path::Path, force: bool) -> Result<(), Error> {
    if !force && tokio::fs::try_exists(file).await? {
        let path = file.to_owned();
        let error = Error::Exists(path);
        return Err(error);
    }
    Ok(())
}

#[tracing::instrument(skip(declaration))]
pub async fn write(
    file: &path::Path,
    declaration: &types::Declaration,
    force: bool,
) -> Result<(), Error> {
    let content = format!("{}\n", serde_json::to_string_pretty(declaration)?);
    let mut destination = tokio::fs::OpenOptions::new()
        .write(true)
        .create(force)
        .create_new(!force)
        .truncate(force)
        .open(file)
        .await?;
    let bytes = content.as_bytes();
    destination.write_all(bytes).await?;
    destination.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn pull_requires_force_to_overwrite_and_round_trips() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let file = directory.path().join("settings.json");
        let declaration =
            serde_json::from_str(include_str!("../../tests/fixtures/declaration.json"))
                .expect("fixture");
        let mut declaration = super::types::normalize(declaration).expect("valid fixture");
        super::write(&file, &declaration, false)
            .await
            .expect("initial pull");
        super::check_destination(&file, false)
            .await
            .expect_err("overwrite requires force");
        super::write(&file, &declaration, false)
            .await
            .expect_err("atomic create prevents overwrite");
        let before = super::read(&file).await.expect("read declaration");
        assert_eq!(before, declaration);
        declaration.settings.description = None;
        super::write(&file, &declaration, true)
            .await
            .expect("forced pull");
        let after = super::read(&file).await.expect("read updated declaration");
        assert_eq!(after, declaration);
    }

    #[tokio::test]
    async fn malformed_or_missing_declarations_fail() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let file = directory.path().join("settings.json");
        super::read(&file).await.expect_err("missing declaration");
        tokio::fs::write(&file, "{}")
            .await
            .expect("write invalid declaration");
        super::read(&file)
            .await
            .expect_err("missing required fields");
    }

    #[tokio::test]
    async fn hyphenated_keys_suggest_underscores_and_ignore_values() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let file = directory.path().join("settings.json");
        let content = include_str!("../../tests/fixtures/declaration.json")
            .replace("vulnerability_alerts", "vulnerability-alerts")
            .replace("automated_security_fixes", "automated-security-fixes");
        tokio::fs::write(&file, content)
            .await
            .expect("write declaration");
        let error = super::read(&file).await.expect_err("hyphenated keys");
        let expected = format!(
            "{} has keys with \"-\"; rename them to match the schema\n  - security.automated-security-fixes → security.automated_security_fixes\n  - security.vulnerability-alerts → security.vulnerability_alerts",
            file.display()
        );
        assert_eq!(error.to_string(), expected);
    }

    #[tokio::test]
    async fn unknown_nested_fields_fail() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let file = directory.path().join("settings.json");
        let mut declaration: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/declaration.json"))
                .expect("fixture");
        let field = "unknown".to_owned();
        let value = serde_json::Value::Bool(true);
        declaration
            .pointer_mut("/settings")
            .and_then(serde_json::Value::as_object_mut)
            .expect("settings object")
            .insert(field, value);
        let content = serde_json::to_string(&declaration).expect("serialize");
        tokio::fs::write(&file, content)
            .await
            .expect("write declaration");
        let error = super::read(&file).await.expect_err("unknown field");
        assert!(
            matches!(error, super::Error::UnknownFields(fields) if fields == "settings.unknown")
        );
    }
}
