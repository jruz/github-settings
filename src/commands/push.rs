use std::time;

use tokio::sync::mpsc;

use super::shared;
use crate::library::{github::api, output, plan};

pub const POLLING: Polling = Polling {
    attempts: 30,
    interval: time::Duration::from_secs(1),
};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Api(#[from] api::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Output(#[from] output::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Shared(#[from] shared::Error),
    #[error("push stopped at {name}")]
    Stopped { name: String },
    #[error("GitHub did not report {expected} for {path} after {attempts} checks")]
    Unsettled {
        attempts: usize,
        expected: serde_json::Value,
        path: String,
    },
}

pub struct Polling {
    pub attempts: usize,
    pub interval: time::Duration,
}

#[tracing::instrument(skip_all)]
pub async fn run(sender: &mpsc::UnboundedSender<String>) -> Result<(), Error> {
    let client = shared::client()?;
    let changes = shared::read_changes(&client).await?;
    let event = output::Event::Planned(&changes);
    output::emit(sender, event)?;
    if !changes.is_empty() {
        apply(&client, &changes, &POLLING, sender).await?;
    }
    Ok(())
}

#[tracing::instrument(skip_all)]
pub async fn apply(
    client: &api::Client,
    changes: &[plan::Change],
    polling: &Polling,
    sender: &mpsc::UnboundedSender<String>,
) -> Result<(), Error> {
    for (applied, change) in changes.iter().enumerate() {
        if let Err(error) = apply_change(client, change, polling).await {
            let failure = output::Failure {
                message: describe(&error),
                name: change.name.clone(),
            };
            let skipped = changes
                .iter()
                .skip(applied + 1)
                .map(|skipped| skipped.name.as_str())
                .collect::<Vec<_>>();
            report_failure(sender, applied, &failure, &skipped)?;
            return Err(Error::Stopped { name: failure.name });
        }
        let event = output::Event::Applied(&change.name);
        output::emit(sender, event)?;
    }
    let summary = output::Summary {
        applied: changes.len(),
        failures: &[],
        skipped: 0,
    };
    let event = output::Event::Pushed(summary);
    output::emit(sender, event)?;
    Ok(())
}

async fn apply_change(
    client: &api::Client,
    change: &plan::Change,
    polling: &Polling,
) -> Result<(), Error> {
    let method = change.method.clone();
    let payload = change.payload.as_ref();
    let response = api::request(client, method, &change.path, payload).await?;
    let applied_method = change.method.clone();
    api::successful(response, applied_method, &change.path).await?;
    if let Some(expected) = &change.settled {
        settle(client, &change.path, expected, polling).await?;
    }
    Ok(())
}

fn report_failure(
    sender: &mpsc::UnboundedSender<String>,
    applied: usize,
    failure: &output::Failure,
    skipped: &[&str],
) -> Result<(), Error> {
    let failed = output::Event::Failed(&failure.name);
    output::emit(sender, failed)?;
    for name in skipped {
        let skipped_change = output::Event::Skipped(name);
        output::emit(sender, skipped_change)?;
    }
    let summary = output::Summary {
        applied,
        failures: std::slice::from_ref(failure),
        skipped: skipped.len(),
    };
    let pushed = output::Event::Pushed(summary);
    output::emit(sender, pushed)?;
    Ok(())
}

fn describe(error: &Error) -> String {
    let Error::Api(error) = error else {
        return error.to_string();
    };
    api::describe(error)
}

#[tracing::instrument(skip(client, polling))]
async fn settle(
    client: &api::Client,
    path: &str,
    expected: &serde_json::Value,
    polling: &Polling,
) -> Result<(), Error> {
    let mut attempts: usize = 1;
    let mut current = read(client, path).await?;
    while !contains(&current, expected) {
        if attempts == polling.attempts {
            return Err(Error::Unsettled {
                attempts,
                expected: expected.clone(),
                path: path.to_owned(),
            });
        }
        tokio::time::sleep(polling.interval).await;
        current = read(client, path).await?;
        attempts += 1;
    }
    Ok(())
}

async fn read(client: &api::Client, path: &str) -> Result<serde_json::Value, Error> {
    let response = api::request(client, reqwest::Method::GET, path, None).await?;
    let response = api::successful(response, reqwest::Method::GET, path).await?;
    let current = response.json().await?;
    Ok(current)
}

fn contains(current: &serde_json::Value, expected: &serde_json::Value) -> bool {
    expected
        .as_object()
        .into_iter()
        .flatten()
        .all(|(key, value)| current.get(key) == Some(value))
}
