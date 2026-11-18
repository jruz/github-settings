use super::{github::api, types};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug)]
pub struct Change {
    pub after: serde_json::Value,
    pub before: serde_json::Value,
    pub method: reqwest::Method,
    pub name: String,
    pub path: String,
    pub payload: Option<serde_json::Value>,
    pub settled: Option<serde_json::Value>,
}

pub fn changes(
    desired: &types::Declaration,
    live: &types::Declaration,
) -> Result<Vec<Change>, Error> {
    let base = format!("repos/{}", desired.repository.0);
    let visibility = visibility(&base, live.settings.private, desired.settings.private)?;
    let visible_settings = types::Settings {
        private: desired.settings.private,
        ..live.settings.clone()
    };
    let repository = difference(
        "settings",
        reqwest::Method::PATCH,
        &base,
        &visible_settings,
        &desired.settings,
    )?;
    let protection = difference(
        "branch_protection",
        if desired.branch_protection.is_some() {
            reqwest::Method::PUT
        } else {
            reqwest::Method::DELETE
        },
        &format!(
            "{base}/branches/{}/protection",
            api::segment(&desired.settings.default_branch)
        ),
        &live.branch_protection,
        &desired.branch_protection,
    )?;
    let topics = difference(
        "topics",
        reqwest::Method::PUT,
        &format!("{base}/topics"),
        &live.topics,
        &desired.topics,
    )?;
    let actions = difference(
        "actions",
        reqwest::Method::PUT,
        &format!("{base}/actions/permissions"),
        &live.actions,
        &desired.actions,
    )?;
    let alerts = toggle(
        "security.vulnerability_alerts",
        &format!("{base}/vulnerability-alerts"),
        live.security.vulnerability_alerts,
        desired.security.vulnerability_alerts,
    )?;
    let fixes = toggle(
        "security.automated_security_fixes",
        &format!("{base}/automated-security-fixes"),
        live.security.automated_security_fixes,
        desired.security.automated_security_fixes,
    )?;
    let (settings, archive) = if desired.settings.archived {
        (None, Some(repository))
    } else {
        (Some(repository), None)
    };
    let mut candidates = vec![visibility];
    candidates.extend(settings);
    candidates.extend([topics, protection, actions]);
    if desired.security.automated_security_fixes {
        candidates.extend([alerts, fixes]);
    } else {
        candidates.extend([fixes, alerts]);
    }
    let upserts = upsert_labels(&base, &desired.labels, &live.labels)?;
    let prunes = prune_labels(&base, &desired.labels, &live.labels)?;
    candidates.extend(upserts);
    candidates.extend(prunes);
    candidates.extend(archive);
    let changes = candidates
        .into_iter()
        .filter(|change| change.before != change.after)
        .collect();
    Ok(changes)
}

fn difference<T>(
    name: &str,
    method: reqwest::Method,
    path: &str,
    before: &T,
    after: &T,
) -> Result<Change, Error>
where
    T: serde::Serialize,
{
    let after = serde_json::to_value(after)?;
    let payload = if method == reqwest::Method::DELETE {
        None
    } else {
        let payload = after.clone();
        Some(payload)
    };
    Ok(Change {
        name: name.to_owned(),
        method,
        path: path.to_owned(),
        before: serde_json::to_value(before)?,
        after,
        payload,
        settled: None,
    })
}

fn visibility(base: &str, before: bool, after: bool) -> Result<Change, Error> {
    let before = serde_json::json!({ "private": before });
    let after = serde_json::json!({ "private": after });
    let mut change = difference(
        "settings.private",
        reqwest::Method::PATCH,
        base,
        &before,
        &after,
    )?;
    change.settled = Some(after);
    Ok(change)
}

fn toggle(name: &str, path: &str, before: bool, after: bool) -> Result<Change, Error> {
    let mut change = difference(
        name,
        if after {
            reqwest::Method::PUT
        } else {
            reqwest::Method::DELETE
        },
        path,
        &before,
        &after,
    )?;
    change.payload = None;
    Ok(change)
}

fn upsert_labels(
    base: &str,
    desired: &[types::Label],
    live: &[types::Label],
) -> Result<Vec<Change>, Error> {
    desired
        .iter()
        .map(|label| {
            let current = live.iter().find(|current| current.name == label.name);
            let (method, path) = if current.is_some() {
                (
                    reqwest::Method::PATCH,
                    format!("{base}/labels/{}", api::segment(&label.name)),
                )
            } else {
                (reqwest::Method::POST, format!("{base}/labels"))
            };
            let payload = serde_json::to_value(label)?;
            Ok(Change {
                name: format!("labels.{}", label.name),
                method,
                path,
                before: serde_json::to_value(current)?,
                after: serde_json::to_value(label)?,
                payload: Some(payload),
                settled: None,
            })
        })
        .collect()
}

fn prune_labels(
    base: &str,
    desired: &[types::Label],
    live: &[types::Label],
) -> Result<Vec<Change>, Error> {
    live.iter()
        .filter(|label| !desired.iter().any(|wanted| wanted.name == label.name))
        .map(|label| {
            Ok(Change {
                name: format!("labels.{}", label.name),
                method: reqwest::Method::DELETE,
                path: format!("{base}/labels/{}", api::segment(&label.name)),
                before: serde_json::to_value(label)?,
                after: serde_json::Value::Null,
                payload: None,
                settled: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::library::types;

    fn fixture() -> types::Declaration {
        let declaration =
            serde_json::from_str(include_str!("../../tests/fixtures/declaration.json"))
                .expect("fixture");
        types::normalize(declaration).expect("valid")
    }

    #[test]
    fn matching_settings_produce_no_changes() {
        assert!(
            super::changes(&fixture(), &fixture())
                .expect("plan")
                .is_empty()
        );
    }

    #[test]
    fn labels_are_created_updated_and_deleted() {
        let desired = fixture();
        let mut live = fixture();
        live.labels = vec![types::Label {
            name: "unmanaged".to_owned(),
            color: "abcdef".to_owned(),
        }];
        let changes = super::changes(&desired, &live).expect("plan");
        let (deletions, creations): (Vec<_>, Vec<_>) = changes
            .iter()
            .partition(|change| change.method == reqwest::Method::DELETE);
        assert_eq!(creations.len(), desired.labels.len());
        assert!(
            creations
                .iter()
                .all(|change| change.method == reqwest::Method::POST)
        );
        assert!(
            matches!(deletions.as_slice(), [change] if change.name == "labels.unmanaged" && change.payload.is_none() && change.after.is_null())
        );
        let mut existing = fixture();
        for label in &mut existing.labels {
            label.color = "ffffff".to_owned();
        }
        assert!(
            super::changes(&desired, &existing)
                .expect("plan")
                .iter()
                .all(|change| change.method == reqwest::Method::PATCH)
        );
    }

    #[test]
    fn protection_can_be_created_and_deleted() {
        let desired = fixture();
        let mut live = fixture();
        live.branch_protection = None;
        let changes = super::changes(&desired, &live).expect("plan");
        assert!(matches!(changes.as_slice(), [change] if change.method == reqwest::Method::PUT));
        let deletion = super::changes(&live, &desired).expect("plan");
        assert!(
            matches!(deletion.as_slice(), [change] if change.method == reqwest::Method::DELETE && change.payload.is_none())
        );
    }

    #[test]
    fn security_changes_have_no_json_body() {
        let desired = fixture();
        let mut live = fixture();
        live.security.vulnerability_alerts = !desired.security.vulnerability_alerts;
        live.security.automated_security_fixes = !desired.security.automated_security_fixes;
        let changes = super::changes(&desired, &live).expect("plan");
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().all(|change| change.payload.is_none()));
    }

    #[test]
    fn branch_names_are_encoded_as_one_path_segment() {
        let mut desired = fixture();
        desired.settings.default_branch = "release/next".to_owned();
        let mut live = desired.clone();
        live.branch_protection = None;
        let changes = super::changes(&desired, &live).expect("plan");
        assert!(
            matches!(changes.as_slice(), [change] if change.path.contains("release%2Fnext/protection"))
        );
    }

    #[test]
    fn visibility_is_changed_alone_before_other_settings() {
        let mut desired = fixture();
        desired.settings.private = false;
        let mut live = fixture();
        live.settings.private = true;
        live.settings.has_wiki = !desired.settings.has_wiki;
        let changes = super::changes(&desired, &live).expect("plan");
        let private = serde_json::json!({"private": false});
        assert!(matches!(
            changes.as_slice(),
            [visibility, repository]
                if visibility.name == "settings.private"
                    && visibility.payload.as_ref() == Some(&private)
                    && visibility.settled.as_ref() == Some(&private)
                    && repository.name == "settings"
                    && repository.before.get("private") == repository.after.get("private")
        ));
    }

    #[test]
    fn archiving_happens_after_other_updates() {
        let mut desired = fixture();
        desired.settings.archived = true;
        let mut live = fixture();
        live.topics.names.clear();
        let changes = super::changes(&desired, &live).expect("plan");
        assert!(
            matches!(changes.as_slice(), [topics, repository] if topics.name == "topics" && repository.name == "settings")
        );
    }
}
