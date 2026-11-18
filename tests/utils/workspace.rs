pub fn command() -> tokio::process::Command {
    tokio::process::Command::new(env!("CARGO_BIN_EXE_github-settings"))
}

pub fn fixture(key: &str) -> serde_json::Value {
    let declaration: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/declaration.json")).expect("fixture");
    declaration.get(key).expect("fixture key").clone()
}

pub async fn create() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("temporary directory");
    git(&directory, &["init", "--quiet"]).await;
    git(
        &directory,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/jruz/agent-config.git",
        ],
    )
    .await;
    let declaration = include_str!("../fixtures/declaration.json");
    let file = directory.path().join("github-settings.json");
    tokio::fs::write(file, declaration)
        .await
        .expect("write declaration");
    directory
}

async fn git(directory: &tempfile::TempDir, arguments: &[&str]) {
    let root = directory.path();
    let status = tokio::process::Command::new("git")
        .current_dir(root)
        .args(arguments)
        .status()
        .await
        .expect("run git");
    assert!(status.success());
}
