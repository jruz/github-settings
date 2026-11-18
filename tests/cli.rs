#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn missing_token_fails_without_network_access() {
        let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_github-settings"))
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

    #[tokio::test]
    async fn help_and_schema_do_not_require_credentials() {
        let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_github-settings"))
            .env_remove("GITHUB_TOKEN")
            .arg("--help")
            .output()
            .await
            .expect("run help");
        assert!(output.status.success());
        let help = String::from_utf8(output.stdout).expect("UTF-8 help");
        assert!(help.contains("pull"));
        assert!(help.contains("plan"));
        assert!(help.contains("push"));
        let schema_output = tokio::process::Command::new(env!("CARGO_BIN_EXE_github-settings"))
            .env_remove("GITHUB_TOKEN")
            .arg("schema")
            .output()
            .await
            .expect("run schema");
        assert!(schema_output.status.success());
        let schema: serde_json::Value =
            serde_json::from_slice(&schema_output.stdout).expect("valid JSON Schema");
        let expected: serde_json::Value =
            serde_json::from_str(include_str!("../github-settings.schema.json"))
                .expect("checked-in schema");
        assert_eq!(schema, expected);
    }
}
