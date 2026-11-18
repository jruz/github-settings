use crate::utils::workspace;

#[tokio::test]
async fn missing_token_fails_without_network_access() {
    let output = workspace::command()
        .env_remove("GITHUB_TOKEN")
        .arg("plan")
        .output()
        .await
        .expect("run CLI");
    assert!(!output.status.success());
    let empty: &[u8] = &[];
    assert_eq!(output.stdout, empty);
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
    assert!(stderr.contains("GITHUB_TOKEN is not set"));
    assert!(stderr.contains("gh auth token"));
}
