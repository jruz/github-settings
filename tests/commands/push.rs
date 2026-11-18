use crate::utils::{mock, workspace};

const REPOSITORY: &str = "repos/jruz/agent-config";

fn read_steps() -> Vec<mock::Step> {
    let settings = workspace::fixture("settings").to_string();
    let labels = workspace::fixture("labels").to_string();
    let protection = serde_json::json!({
        "allow_deletions": {"enabled": false},
        "allow_force_pushes": {"enabled": true},
        "enforce_admins": {"enabled": true},
        "required_linear_history": {"enabled": true},
        "required_status_checks": null
    });
    vec![
        mock::get(REPOSITORY, 200, &settings),
        mock::get(
            &format!("{REPOSITORY}/branches/main/protection"),
            200,
            &protection.to_string(),
        ),
        mock::get(&format!("{REPOSITORY}/topics"), 200, r#"{"names":[]}"#),
        mock::get(
            &format!("{REPOSITORY}/actions/permissions"),
            200,
            r#"{"enabled":true}"#,
        ),
        mock::get(&format!("{REPOSITORY}/vulnerability-alerts"), 204, ""),
        mock::get(
            &format!("{REPOSITORY}/automated-security-fixes"),
            200,
            r#"{"enabled":false}"#,
        ),
        mock::get(
            &format!("{REPOSITORY}/labels?per_page=100&page=1"),
            200,
            &labels,
        ),
    ]
}

fn topics_step() -> mock::Step {
    let names = [
        "agent-configuration",
        "claude-code",
        "codex",
        "dotfiles",
        "pi",
    ];
    let payload = serde_json::json!({ "names": names });
    mock::put(&format!("{REPOSITORY}/topics"), payload, 200, "{}")
}

async fn push(steps: Vec<mock::Step>, state: &std::path::Path) -> std::process::Output {
    let directory = workspace::create().await;
    let (base, server) = mock::serve_all(steps).await;
    let root = directory.path();
    let output = workspace::command()
        .current_dir(root)
        .env("GITHUB_TOKEN", "mock-token")
        .env("GITHUB_API_URL", base)
        .env("XDG_STATE_HOME", state)
        .arg("push")
        .output()
        .await
        .expect("run push");
    server
        .await
        .expect("server task")
        .expect("requests matched");
    output
}

#[tokio::test]
async fn failed_push_prints_only_its_report() {
    let state = tempfile::tempdir().expect("state directory");
    let actions = mock::put(
        &format!("{REPOSITORY}/actions/permissions"),
        serde_json::json!({"enabled": false}),
        403,
        r#"{"message":"Resource not accessible by token"}"#,
    );
    let steps = read_steps()
        .into_iter()
        .chain([topics_step(), actions])
        .collect();
    let state_path = state.path();
    let output = push(steps, state_path).await;
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    assert!(stdout.ends_with(
        "\u{2713} topics\n\u{2717} actions\n\nerrors:\n- actions: Resource not accessible by token\n  help: GITHUB_TOKEN needs admin access to this repository\n\n1 applied, 1 failed\n"
    ));
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 stderr");
    assert_eq!(stderr, "");
    assert!(!output.status.success());
    let log = state_path.join("github-settings").join("errors.jsonl");
    let log = tokio::fs::read_to_string(log).await.expect("error log");
    assert!(log.lines().any(|line| line.contains(r#""key":"actions""#)));
}

#[tokio::test]
async fn successful_push_ends_with_its_summary() {
    let state = tempfile::tempdir().expect("state directory");
    let mut steps = read_steps();
    for step in &mut steps {
        if step.path.ends_with("/actions/permissions") {
            step.body = r#"{"enabled":false}"#.to_owned();
        }
    }
    let topics = topics_step();
    steps.push(topics);
    let state_path = state.path();
    let output = push(steps, state_path).await;
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    assert!(stdout.ends_with("\u{2713} topics\n\n1 applied\n"));
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 stderr");
    assert_eq!(stderr, "");
    assert!(output.status.success());
}
