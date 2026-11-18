#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("invalid label color: {0}; expected six hexadecimal digits")]
    Color(String),
    #[error("duplicate label: {0}")]
    DuplicateLabel(String),
    #[error("default_branch cannot be empty")]
    EmptyBranch,
    #[error("label names cannot be empty")]
    EmptyLabel,
    #[error("invalid repository: {0}; expected owner/repo")]
    Repository(String),
    #[error("required_approving_review_count must be at most 6")]
    ReviewCount,
    #[error("invalid topic: {0}")]
    Topic(String),
    #[error("a repository can have at most 20 topics")]
    TopicCount,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct Repository(#[schemars(regex(pattern = r"^[\w.-]+/[\w.-]+$"))] pub String);

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[schemars(deny_unknown_fields)]
pub struct Declaration {
    pub actions: Enabled,
    #[serde(deserialize_with = "nullable")]
    pub branch_protection: Option<Protection>,
    pub labels: Vec<Label>,
    pub repository: Repository,
    pub security: Security,
    pub settings: Settings,
    pub topics: Topics,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[schemars(deny_unknown_fields)]
pub struct Settings {
    pub allow_auto_merge: bool,
    pub allow_merge_commit: bool,
    pub allow_rebase_merge: bool,
    pub allow_squash_merge: bool,
    pub archived: bool,
    #[schemars(length(min = 1))]
    pub default_branch: String,
    pub delete_branch_on_merge: bool,
    #[serde(deserialize_with = "nullable")]
    pub description: Option<String>,
    pub has_issues: bool,
    pub has_projects: bool,
    pub has_wiki: bool,
    pub is_template: bool,
    pub private: bool,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[schemars(deny_unknown_fields)]
pub struct Topics {
    #[schemars(length(max = 20))]
    pub names: Vec<Topic>,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct Topic(#[schemars(regex(pattern = "^[a-z0-9][a-z0-9-]{0,49}$"))] pub String);

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[schemars(deny_unknown_fields)]
pub struct Protection {
    pub allow_deletions: bool,
    pub allow_force_pushes: bool,
    pub enforce_admins: bool,
    pub required_linear_history: bool,
    #[serde(deserialize_with = "nullable")]
    pub required_pull_request_reviews: Option<ReviewRequirements>,
    #[serde(deserialize_with = "nullable")]
    pub required_status_checks: Option<StatusChecks>,
    #[serde(deserialize_with = "nullable")]
    pub restrictions: Option<Restrictions>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[schemars(deny_unknown_fields)]
pub struct ReviewRequirements {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bypass_pull_request_allowances: Option<Restrictions>,
    pub dismiss_stale_reviews: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dismissal_restrictions: Option<Restrictions>,
    pub require_code_owner_reviews: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_last_push_approval: Option<bool>,
    #[schemars(range(max = 6))]
    pub required_approving_review_count: u8,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[schemars(deny_unknown_fields)]
pub struct Restrictions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apps: Option<Vec<String>>,
    pub teams: Vec<String>,
    pub users: Vec<String>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[schemars(deny_unknown_fields)]
pub struct StatusChecks {
    pub contexts: Vec<String>,
    pub strict: bool,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[schemars(deny_unknown_fields)]
pub struct Enabled {
    pub enabled: bool,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[schemars(deny_unknown_fields)]
pub struct Security {
    #[serde(rename = "automated-security-fixes")]
    pub automated_security_fixes: bool,
    #[serde(rename = "vulnerability-alerts")]
    pub vulnerability_alerts: bool,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[schemars(deny_unknown_fields)]
pub struct Label {
    #[schemars(regex(pattern = "^[0-9a-fA-F]{6}$"))]
    pub color: String,
    #[schemars(length(min = 1))]
    pub name: String,
}

pub fn schema() -> schemars::Schema {
    let settings = schemars::generate::SchemaSettings::draft2020_12().for_serialize();
    let generator = settings.into_generator();
    generator.into_root_schema_for::<Declaration>()
}

fn nullable<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(deserializer)
}

pub fn repository(value: &str) -> Result<Repository, Error> {
    let Some((owner, name)) = value.split_once('/') else {
        let value = value.to_owned();
        let error = Error::Repository(value);
        return Err(error);
    };
    let valid = |part: &str| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    };
    if !valid(owner) || !valid(name) {
        let value = value.to_owned();
        let error = Error::Repository(value);
        return Err(error);
    }
    let value = value.to_owned();
    let repository = Repository(value);
    Ok(repository)
}

pub fn normalize(mut declaration: Declaration) -> Result<Declaration, Error> {
    repository(&declaration.repository.0)?;
    if declaration.settings.default_branch.is_empty() {
        return Err(Error::EmptyBranch);
    }
    normalize_topics(&mut declaration.topics)?;
    if let Some(protection) = &mut declaration.branch_protection {
        normalize_protection(protection)?;
    }
    normalize_labels(&mut declaration.labels)?;
    Ok(declaration)
}

fn normalize_topics(topics: &mut Topics) -> Result<(), Error> {
    if topics.names.len() > 20 {
        return Err(Error::TopicCount);
    }
    for Topic(topic) in &topics.names {
        if topic.is_empty()
            || topic.len() > 50
            || !topic.starts_with(|character: char| {
                character.is_ascii_lowercase() || character.is_ascii_digit()
            })
            || !topic
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            let topic = topic.clone();
            let error = Error::Topic(topic);
            return Err(error);
        }
    }
    topics.names.sort();
    topics.names.dedup();
    Ok(())
}

fn normalize_protection(protection: &mut Protection) -> Result<(), Error> {
    if let Some(checks) = &mut protection.required_status_checks {
        checks.contexts.sort();
        checks.contexts.dedup();
    }
    if let Some(restrictions) = &mut protection.restrictions {
        normalize_restrictions(restrictions);
    }
    if let Some(reviews) = &mut protection.required_pull_request_reviews {
        if reviews.required_approving_review_count > 6 {
            return Err(Error::ReviewCount);
        }
        if let Some(restrictions) = &mut reviews.dismissal_restrictions {
            normalize_restrictions(restrictions);
        }
        if let Some(restrictions) = &mut reviews.bypass_pull_request_allowances {
            normalize_restrictions(restrictions);
        }
    }
    Ok(())
}

fn normalize_restrictions(restrictions: &mut Restrictions) {
    restrictions.users.sort();
    restrictions.users.dedup();
    restrictions.teams.sort();
    restrictions.teams.dedup();
    if let Some(apps) = &mut restrictions.apps {
        apps.sort();
        apps.dedup();
    }
}

fn normalize_labels(labels: &mut [Label]) -> Result<(), Error> {
    labels.sort_by(|left, right| left.name.cmp(&right.name));
    for label in labels.iter_mut() {
        if label.name.is_empty() {
            return Err(Error::EmptyLabel);
        }
        if label.color.len() != 6 || !label.color.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            let color = label.color.clone();
            let error = Error::Color(color);
            return Err(error);
        }
        label.color.make_ascii_lowercase();
    }
    for pair in labels.windows(2) {
        if let [left, right] = pair
            && left.name == right.name
        {
            let name = left.name.clone();
            let error = Error::DuplicateLabel(name);
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Declaration, normalize, repository};

    fn fixture() -> Declaration {
        serde_json::from_str(include_str!("../../tests/fixtures/declaration.json"))
            .expect("valid fixture")
    }

    #[test]
    fn repository_requires_owner_and_name() {
        for invalid in ["repo", "owner/", "/repo", "a/b/c", "a/..", "a/repo?x"] {
            repository(invalid).expect_err("invalid repository");
        }
        repository("owner/repo-name").expect("valid repository");
    }

    #[test]
    fn missing_fields_are_rejected() {
        let fixture = fixture();
        let mut value = serde_json::to_value(fixture).expect("fixture serializes");
        value
            .as_object_mut()
            .expect("object")
            .remove("branch_protection");
        serde_json::from_value::<Declaration>(value).expect_err("required nullable field");
    }

    #[test]
    fn normalization_ignores_order_and_color_case() {
        let mut desired = fixture();
        desired.topics.names.reverse();
        desired.labels.reverse();
        for label in &mut desired.labels {
            label.color.make_ascii_uppercase();
        }
        let original = fixture();
        let expected = normalize(original).expect("valid");
        assert_eq!(normalize(desired).expect("valid"), expected);
    }

    #[test]
    fn invalid_topics_colors_and_duplicate_labels_fail() {
        let mut bad_topics = fixture();
        let invalid = "Invalid Topic".to_owned();
        let invalid = super::Topic(invalid);
        bad_topics.topics.names.push(invalid);
        normalize(bad_topics).expect_err("invalid topic");
        let mut bad_colors = fixture();
        bad_colors.labels.push(super::Label {
            name: "bad".to_owned(),
            color: "red".to_owned(),
        });
        normalize(bad_colors).expect_err("invalid color");
        let mut duplicates = fixture();
        let original = fixture();
        duplicates.labels.extend(original.labels);
        normalize(duplicates).expect_err("duplicate labels");
    }
}
