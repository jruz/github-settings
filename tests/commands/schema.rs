use crate::utils::workspace;

#[tokio::test]
async fn help_and_schema_do_not_require_credentials() {
    let output = workspace::command()
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
    let schema_output = workspace::command()
        .env_remove("GITHUB_TOKEN")
        .arg("schema")
        .output()
        .await
        .expect("run schema");
    assert!(schema_output.status.success());
    let schema: serde_json::Value =
        serde_json::from_slice(&schema_output.stdout).expect("valid JSON Schema");
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../github-settings.schema.json"))
            .expect("checked-in schema");
    assert_eq!(schema, expected);
}
