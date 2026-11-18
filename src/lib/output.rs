use std::{collections::BTreeSet, io::IsTerminal as _};

use serde_json::Value;
use tokio::sync::mpsc;

use super::{plan, types};

const COLORED: Palette = Palette {
    bold: "\x1b[1m",
    dim: "\x1b[2m",
    green: "\x1b[32m",
    red: "\x1b[31m",
    reset: "\x1b[0m",
    yellow: "\x1b[33m",
};

const PLAIN: Palette = Palette {
    bold: "",
    dim: "",
    green: "",
    red: "",
    reset: "",
    yellow: "",
};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    Channel(#[from] mpsc::error::SendError<String>),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Copy)]
pub enum Event<'event> {
    Applied(&'event str),
    Failed(&'event str),
    Planned(&'event [plan::Change]),
    Pulled(&'event std::path::Path),
    Pushed(Summary<'event>),
    Schema,
    Skipped(&'event str),
}

pub struct Failure {
    pub message: String,
    pub name: String,
}

#[derive(Clone, Copy)]
pub struct Summary<'summary> {
    pub applied: usize,
    pub failures: &'summary [Failure],
    pub skipped: usize,
}

#[derive(Clone, Copy)]
enum Kind {
    Create,
    Delete,
    Update,
}

#[derive(Clone, Copy)]
struct Palette {
    bold: &'static str,
    dim: &'static str,
    green: &'static str,
    red: &'static str,
    reset: &'static str,
    yellow: &'static str,
}

pub fn emit(sender: &mpsc::UnboundedSender<String>, event: Event<'_>) -> Result<(), Error> {
    let message = render(event)?;
    sender.send(message)?;
    Ok(())
}

fn render(event: Event<'_>) -> Result<String, Error> {
    let message = match event {
        Event::Applied(name) => {
            let colors = palette();
            format!("{}✓{} {name}", colors.green, colors.reset)
        }
        Event::Failed(name) => {
            let colors = palette();
            format!("{}✗{} {name}", colors.red, colors.reset)
        }
        Event::Planned([]) => "GitHub settings are up to date".to_owned(),
        Event::Planned(changes) => {
            let colors = palette();
            render_plan(changes, colors)
        }
        Event::Pulled(file) => format!("Pulled settings to {}", file.display()),
        Event::Pushed(summary) => {
            let colors = palette();
            render_push(summary, colors)
        }
        Event::Schema => {
            let schema = types::schema();
            serde_json::to_string_pretty(&schema)?
        }
        Event::Skipped(name) => {
            let colors = palette();
            format!("{}○ {name}{}", colors.dim, colors.reset)
        }
    };
    Ok(message)
}

fn render_push(summary: Summary<'_>, palette: Palette) -> String {
    let errors = summary
        .failures
        .iter()
        .map(|failure| format!("- {}: {}", failure.name, failure.message))
        .collect::<Vec<_>>();
    let errors = if errors.is_empty() {
        String::new()
    } else {
        format!("\nerrors:\n{}\n", errors.join("\n"))
    };
    let applied = format!(
        "{}{} applied{}",
        palette.green, summary.applied, palette.reset
    );
    let incomplete = [
        (summary.failures.len(), "failed", palette.red),
        (summary.skipped, "skipped", palette.dim),
    ]
    .into_iter()
    .filter(|(count, _, _)| *count > 0)
    .map(|(count, label, color)| format!("{color}{count} {label}{}", palette.reset));
    let totals = std::iter::once(applied)
        .chain(incomplete)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{errors}\n{totals}")
}

fn palette() -> Palette {
    if std::io::stdout().is_terminal() {
        COLORED
    } else {
        PLAIN
    }
}

fn render_plan(changes: &[plan::Change], palette: Palette) -> String {
    let blocks = changes
        .iter()
        .map(|change| render_change(change, palette))
        .collect::<Vec<_>>();
    format!(
        "{}\n\n{}",
        blocks.join("\n\n"),
        render_summary(changes, palette)
    )
}

fn render_summary(changes: &[plan::Change], palette: Palette) -> String {
    let count = |wanted: fn(Kind) -> bool| {
        changes
            .iter()
            .map(kind)
            .filter(|kind| wanted(*kind))
            .count()
    };
    let creates = count(|kind| matches!(kind, Kind::Create));
    let updates = count(|kind| matches!(kind, Kind::Update));
    let deletes = count(|kind| matches!(kind, Kind::Delete));
    format!(
        "{bold}Plan:{reset} {green}{creates} to create{reset}, {yellow}{updates} to update{reset}, {red}{deletes} to delete{reset}",
        bold = palette.bold,
        green = palette.green,
        yellow = palette.yellow,
        red = palette.red,
        reset = palette.reset,
    )
}

fn kind(change: &plan::Change) -> Kind {
    match (&change.before, &change.after) {
        (Value::Null, _) => Kind::Create,
        (_, Value::Null) => Kind::Delete,
        _ => Kind::Update,
    }
}

fn render_change(change: &plan::Change, palette: Palette) -> String {
    let (symbol, color) = match kind(change) {
        Kind::Create => ("+", palette.green),
        Kind::Update => ("~", palette.yellow),
        Kind::Delete => ("-", palette.red),
    };
    let header = format!(
        "{color}{symbol}{reset} {bold}{name}{reset} {dim}{method} {path}{reset}",
        bold = palette.bold,
        dim = palette.dim,
        reset = palette.reset,
        name = change.name,
        method = change.method,
        path = change.path,
    );
    let fields = field_changes(&change.before, &change.after)
        .into_iter()
        .map(|(key, before, after)| render_field(&key, before, after, palette));
    std::iter::once(header)
        .chain(fields)
        .collect::<Vec<_>>()
        .join("\n")
}

fn field_changes<'value>(
    before: &'value Value,
    after: &'value Value,
) -> Vec<(String, Option<&'value Value>, Option<&'value Value>)> {
    if !before.is_object() && !after.is_object() {
        return vec![(String::new(), non_null(before), non_null(after))];
    }
    let keys = |value: &'value Value| {
        value
            .as_object()
            .into_iter()
            .flat_map(|fields| fields.keys())
    };
    let after_keys = keys(after);
    keys(before)
        .chain(after_keys)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|key| (key.clone(), before.get(key), after.get(key)))
        .filter(|(_, before, after)| before != after)
        .collect()
}

fn non_null(value: &Value) -> Option<&Value> {
    Some(value).filter(|value| !value.is_null())
}

fn render_field(
    key: &str,
    before: Option<&Value>,
    after: Option<&Value>,
    palette: Palette,
) -> String {
    let label = if key.is_empty() {
        "    ".to_owned()
    } else {
        format!("    {key}: ")
    };
    let value = match (before, after) {
        (Some(old), Some(new)) => format!(
            "{red}{old}{reset} → {green}{new}{reset}",
            red = palette.red,
            green = palette.green,
            reset = palette.reset,
        ),
        (None, Some(new)) => format!("{}{new}{}", palette.green, palette.reset),
        (Some(old), None) => format!("{}{old}{}", palette.red, palette.reset),
        (None, None) => String::new(),
    };
    format!("{label}{value}")
}
