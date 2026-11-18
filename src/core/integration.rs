#[cfg(test)]
mod tests {
    use tokio::{
        io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _},
        net,
        sync::mpsc,
        task,
    };

    use crate::{
        core::{plan, push},
        libs::github::{api, live},
        utils::types,
    };

    const FAST: push::Polling = push::Polling {
        attempts: 2,
        interval: std::time::Duration::from_millis(1),
    };

    #[derive(Debug, thiserror::Error)]
    enum Error {
        #[error(transparent)]
        Io(#[from] std::io::Error),
        #[error(transparent)]
        Json(#[from] serde_json::Error),
    }

    struct Step {
        body: String,
        method: String,
        path: String,
        payload: Option<serde_json::Value>,
        status: u16,
    }

    fn get(path: &str, status: u16, body: &str) -> Step {
        Step {
            body: body.to_owned(),
            method: "GET".to_owned(),
            path: path.to_owned(),
            payload: None,
            status,
        }
    }

    fn events(receiver: &mut mpsc::UnboundedReceiver<String>) -> Vec<String> {
        let count = receiver.len();
        std::iter::repeat_with(|| receiver.try_recv().expect("queued event"))
            .take(count)
            .collect()
    }

    fn cause(error: &live::Error) -> (String, Option<String>) {
        let source = miette::Diagnostic::diagnostic_source(error).expect("rejected request");
        let help = source.help().map(|help| help.to_string());
        (source.to_string(), help)
    }

    fn applied(change: &plan::Change) -> Step {
        Step {
            body: "{}".to_owned(),
            method: change.method.to_string(),
            path: change.path.clone(),
            payload: change.payload.clone(),
            status: 200,
        }
    }

    async fn mock(steps: Vec<Step>) -> (String, task::JoinHandle<Result<(), Error>>) {
        let listener = net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("mock listener");
        let address = listener.local_addr().expect("mock address");
        let base = format!("http://{address}/");
        let task = tokio::spawn(async move {
            for step in steps {
                serve(&listener, &step).await?;
            }
            Ok(())
        });
        (base, task)
    }

    async fn serve(listener: &net::TcpListener, step: &Step) -> Result<(), Error> {
        let (stream, _) = listener.accept().await?;
        let mut reader = tokio::io::BufReader::new(stream);
        let mut first = String::new();
        reader.read_line(&mut first).await?;
        assert_eq!(
            first.trim(),
            format!("{} /{} HTTP/1.1", step.method, step.path)
        );
        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            if line == "\r\n" {
                break;
            }
            let line = line.to_ascii_lowercase();
            headers.push(line);
        }
        assert!(
            headers
                .iter()
                .any(|header| header.trim() == "authorization: bearer mock-token")
        );
        assert!(
            headers
                .iter()
                .any(|header| header.trim() == "accept: application/vnd.github+json")
        );
        assert!(
            headers
                .iter()
                .any(|header| header.trim() == "x-github-api-version: 2022-11-28")
        );
        assert!(
            headers
                .iter()
                .any(|header| header.starts_with("user-agent: github-settings/"))
        );
        let length = headers
            .iter()
            .find_map(|header| header.strip_prefix("content-length: "));
        if let Some(length) = length.filter(|length| length.trim() != "0") {
            let length: usize = length.trim().parse().expect("content length");
            let mut body = vec![0; length];
            reader.read_exact(&mut body).await?;
            let body: serde_json::Value = serde_json::from_slice(&body)?;
            assert_eq!(Some(body), step.payload);
        } else {
            assert!(step.payload.is_none());
        }
        let response = format!(
            "HTTP/1.1 {} Mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            step.status,
            step.body.len(),
            step.body
        );
        let response = response.as_bytes();
        reader.get_mut().write_all(response).await?;
        Ok(())
    }

    fn fixture() -> types::Declaration {
        let declaration =
            serde_json::from_str(include_str!("../../tests/fixtures/declaration.json"))
                .expect("fixture");
        types::normalize(declaration).expect("valid fixture")
    }

    fn read_steps(labels: &str) -> Vec<Step> {
        let fixture = fixture();
        let settings = serde_json::to_string(&fixture.settings).expect("settings");
        let topics = serde_json::to_string(&fixture.topics).expect("topics");
        let protection = serde_json::json!({
            "allow_deletions": {"enabled": false},
            "allow_force_pushes": {"enabled": true},
            "enforce_admins": {"enabled": true},
            "required_linear_history": {"enabled": true},
            "required_status_checks": null
        })
        .to_string();
        vec![
            get("repos/jruz/agent-config", 200, &settings),
            get(
                "repos/jruz/agent-config/branches/main/protection",
                200,
                &protection,
            ),
            get("repos/jruz/agent-config/topics", 200, &topics),
            get(
                "repos/jruz/agent-config/actions/permissions",
                200,
                r#"{"enabled":false}"#,
            ),
            get("repos/jruz/agent-config/vulnerability-alerts", 204, ""),
            get(
                "repos/jruz/agent-config/automated-security-fixes",
                200,
                r#"{"enabled":false}"#,
            ),
            get(
                "repos/jruz/agent-config/labels?per_page=100&page=1",
                200,
                labels,
            ),
        ]
    }

    #[tokio::test]
    async fn pull_and_plan_round_trip_without_any_writes() {
        let desired = fixture();
        let labels = serde_json::to_string(&desired.labels).expect("labels");
        let steps = read_steps(&labels);
        let (base, server) = mock(steps).await;
        let client = api::client("mock-token", &base).expect("client");
        let current = live::read(&client, &desired.repository, None)
            .await
            .expect("live settings");
        let changes = plan::changes(&desired, &current).expect("plan");
        assert!(changes.is_empty());
        server
            .await
            .expect("server task")
            .expect("requests matched");
    }

    #[tokio::test]
    async fn labels_are_paginated() {
        let first: Vec<_> = (0..100)
            .map(|index: usize| types::Label {
                color: "abcdef".to_owned(),
                name: format!("label-{index}"),
            })
            .collect();
        let first = serde_json::to_string(&first).expect("first page");
        let mut steps = read_steps(&first);
        let second_page = get(
            "repos/jruz/agent-config/labels?per_page=100&page=2",
            200,
            r#"[{"name":"last","color":"ABCDEF"}]"#,
        );
        steps.push(second_page);
        let (base, server) = mock(steps).await;
        let client = api::client("mock-token", &base).expect("client");
        let desired = fixture();
        let current = live::read(&client, &desired.repository, None)
            .await
            .expect("live settings");
        assert_eq!(current.labels.len(), 101);
        assert!(
            current
                .labels
                .iter()
                .any(|label| label.name == "last" && label.color == "abcdef")
        );
        server
            .await
            .expect("server task")
            .expect("requests matched");
    }

    #[tokio::test]
    async fn forbidden_responses_are_not_treated_as_disabled() {
        let steps = vec![get(
            "repos/jruz/agent-config",
            403,
            r#"{"message":"Must have admin rights to Repository."}"#,
        )];
        let (base, server) = mock(steps).await;
        let client = api::client("mock-token", &base).expect("client");
        let desired = fixture();
        let error = live::read(&client, &desired.repository, None)
            .await
            .expect_err("forbidden");
        assert_eq!(
            error.to_string(),
            "could not read settings for jruz/agent-config"
        );
        assert_eq!(
            cause(&error),
            (
                "Must have admin rights to Repository.".to_owned(),
                Some("GITHUB_TOKEN needs admin access to this repository".to_owned())
            )
        );
        server
            .await
            .expect("server task")
            .expect("requests matched");
    }

    fn forbidden_protection_steps(private: bool, message: &str) -> Vec<Step> {
        let mut declaration = fixture();
        declaration.settings.private = private;
        let settings = serde_json::to_string(&declaration.settings).expect("settings");
        let labels = serde_json::to_string(&declaration.labels).expect("labels");
        let mut steps = read_steps(&labels);
        for step in &mut steps {
            if step.path == "repos/jruz/agent-config" {
                step.body = settings.clone();
            }
            if step.path.ends_with("/protection") {
                step.status = 403;
                step.body = serde_json::json!({"message": message}).to_string();
            }
        }
        steps
    }

    #[tokio::test]
    async fn public_visibility_is_applied_before_plan_restricted_protection() {
        let restricted =
            "Upgrade to GitHub Pro or make this repository public to enable this feature.";
        let steps = forbidden_protection_steps(true, restricted);
        let (base, server) = mock(steps).await;
        let client = api::client("mock-token", &base).expect("client");
        let mut desired = fixture();
        desired.settings.private = false;
        desired.settings.has_wiki = !desired.settings.has_wiki;
        let current = live::read(&client, &desired.repository, None)
            .await
            .expect("private repository settings");
        assert!(current.branch_protection.is_none());
        server
            .await
            .expect("server task")
            .expect("requests matched");
        let changes = plan::changes(&desired, &current).expect("plan");
        let names = changes
            .iter()
            .map(|change| change.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["settings.private", "settings", "branch_protection"]);
        let mut push_steps: Vec<_> = changes.iter().map(applied).collect();
        let polls = [
            get("repos/jruz/agent-config", 200, r#"{"private":true}"#),
            get("repos/jruz/agent-config", 200, r#"{"private":false}"#),
        ];
        push_steps.splice(1..1, polls);
        let (push_base, push_server) = mock(push_steps).await;
        let push_client = api::client("mock-token", &push_base).expect("client");
        let (sender, mut receiver) = mpsc::unbounded_channel();
        push::apply(&push_client, &changes, &FAST, &sender)
            .await
            .expect("push");
        assert_eq!(
            events(&mut receiver),
            [
                "\u{2713} settings.private",
                "\u{2713} settings",
                "\u{2713} branch_protection",
                "\n3 applied"
            ]
        );
        push_server
            .await
            .expect("server task")
            .expect("requests matched");
    }

    #[tokio::test]
    async fn unsettled_visibility_stops_the_push() {
        let mut desired = fixture();
        desired.settings.private = false;
        let mut current = fixture();
        current.settings.private = true;
        current.labels.clear();
        let changes = plan::changes(&desired, &current).expect("plan");
        let stale = get("repos/jruz/agent-config", 200, r#"{"private":true}"#);
        let stale_again = get("repos/jruz/agent-config", 200, r#"{"private":true}"#);
        let visibility = changes.first().expect("visibility change");
        let steps = vec![applied(visibility), stale, stale_again];
        let (base, server) = mock(steps).await;
        let client = api::client("mock-token", &base).expect("client");
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let error = push::apply(&client, &changes, &FAST, &sender)
            .await
            .expect_err("visibility never settled");
        assert_eq!(error.to_string(), "push stopped at settings.private");
        let events = events(&mut receiver);
        assert_eq!(
            events.first().map(String::as_str),
            Some("\u{2717} settings.private")
        );
        let report = events.last().expect("push report");
        assert!(report.contains(
            "- settings.private: GitHub still reports the previous value after 2 checks\n  help: wait a minute and run push again"
        ));
        let skipped = changes.len() - 1;
        assert!(report.ends_with(&format!("0 applied, 1 failed, {skipped} skipped")));
        server
            .await
            .expect("server task")
            .expect("requests matched");
    }

    #[tokio::test]
    async fn unrelated_or_public_protection_forbidden_responses_still_fail() {
        let restricted =
            "Upgrade to GitHub Pro or make this repository public to enable this feature.";
        for (private, message) in [
            (true, "Resource not accessible by token"),
            (false, restricted),
        ] {
            let steps = forbidden_protection_steps(private, message)
                .into_iter()
                .take(2)
                .collect();
            let (base, server) = mock(steps).await;
            let client = api::client("mock-token", &base).expect("client");
            let desired = fixture();
            let result = live::read(&client, &desired.repository, None).await;
            let error = result.expect_err("forbidden");
            assert_eq!(
                error.to_string(),
                "could not read branch_protection for jruz/agent-config"
            );
            assert_eq!(cause(&error).0, message);
            server
                .await
                .expect("server task")
                .expect("requests matched");
        }
    }

    #[tokio::test]
    async fn push_sends_planned_payloads_and_stops_on_failure() {
        let desired = fixture();
        let mut current = fixture();
        current.topics.names.clear();
        current.actions.enabled = true;
        current.labels.clear();
        let changes = plan::changes(&desired, &current).expect("plan");
        let steps = changes
            .iter()
            .take(2)
            .enumerate()
            .map(|(index, change)| Step {
                body: if index == 0 {
                    "{}".to_owned()
                } else {
                    r#"{"message":"Resource not accessible by token"}"#.to_owned()
                },
                method: "PUT".to_owned(),
                path: change.path.clone(),
                payload: change.payload.clone(),
                status: if index == 0 { 200 } else { 403 },
            })
            .collect();
        let (base, server) = mock(steps).await;
        let client = api::client("mock-token", &base).expect("client");
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let error = push::apply(&client, &changes, &FAST, &sender)
            .await
            .expect_err("second update rejected");
        assert_eq!(error.to_string(), "push stopped at actions");
        let skipped = changes
            .iter()
            .skip(2)
            .map(|change| format!("○ {}", change.name))
            .collect::<Vec<_>>();
        let summary = format!(
            "\nerrors:\n- actions: Resource not accessible by token\n  help: GITHUB_TOKEN needs admin access to this repository\n\n1 applied, 1 failed, {} skipped",
            skipped.len()
        );
        let expected = ["\u{2713} topics".to_owned(), "\u{2717} actions".to_owned()]
            .into_iter()
            .chain(skipped)
            .chain([summary])
            .collect::<Vec<_>>();
        assert_eq!(events(&mut receiver), expected);
        server
            .await
            .expect("server task")
            .expect("requests matched");
    }

    #[tokio::test]
    async fn push_applies_every_planned_change() {
        let desired = fixture();
        let mut current = fixture();
        current.settings.has_wiki = true;
        current.topics.names.clear();
        current.branch_protection = None;
        current.actions.enabled = true;
        current.security.vulnerability_alerts = false;
        current.security.automated_security_fixes = true;
        current.labels.clear();
        let changes = plan::changes(&desired, &current).expect("plan");
        let steps = changes
            .iter()
            .map(|change| Step {
                body: String::new(),
                method: change.method.to_string(),
                path: change.path.clone(),
                payload: change.payload.clone(),
                status: 204,
            })
            .collect();
        let (base, server) = mock(steps).await;
        let client = api::client("mock-token", &base).expect("client");
        let (sender, mut receiver) = mpsc::unbounded_channel();
        push::apply(&client, &changes, &FAST, &sender)
            .await
            .expect("push succeeded");
        let expected = changes
            .iter()
            .map(|change| format!("✓ {}", change.name))
            .chain([format!("\n{} applied", changes.len())])
            .collect::<Vec<_>>();
        assert_eq!(events(&mut receiver), expected);
        server
            .await
            .expect("server task")
            .expect("requests matched");
    }

    #[tokio::test]
    async fn absent_protection_and_security_are_read_as_disabled() {
        let desired = fixture();
        let labels = serde_json::to_string(&desired.labels).expect("labels");
        let mut steps = read_steps(&labels);
        for step in &mut steps {
            if step.path.ends_with("/protection")
                || step.path.ends_with("vulnerability-alerts")
                || step.path.ends_with("automated-security-fixes")
            {
                step.status = 404;
                step.body = "{}".to_owned();
            }
        }
        let (base, server) = mock(steps).await;
        let client = api::client("mock-token", &base).expect("client");
        let current = live::read(&client, &desired.repository, None)
            .await
            .expect("live settings");
        assert!(current.branch_protection.is_none());
        assert!(!current.security.vulnerability_alerts);
        assert!(!current.security.automated_security_fixes);
        server
            .await
            .expect("server task")
            .expect("requests matched");
    }

    #[tokio::test]
    async fn invalid_json_is_reported() {
        let steps = vec![get("repos/jruz/agent-config", 200, "not json")];
        let (base, server) = mock(steps).await;
        let client = api::client("mock-token", &base).expect("client");
        let desired = fixture();
        live::read(&client, &desired.repository, None)
            .await
            .expect_err("invalid JSON");
        server
            .await
            .expect("server task")
            .expect("requests matched");
    }
}
