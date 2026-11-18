use crate::library::types;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(serde::Deserialize)]
struct Live {
    allow_deletions: types::Enabled,
    allow_force_pushes: types::Enabled,
    enforce_admins: types::Enabled,
    required_linear_history: types::Enabled,
    required_pull_request_reviews: Option<ReviewRequirements>,
    required_status_checks: Option<types::StatusChecks>,
    restrictions: Option<Restrictions>,
}

#[derive(serde::Deserialize)]
struct ReviewRequirements {
    bypass_pull_request_allowances: Option<Restrictions>,
    dismiss_stale_reviews: bool,
    dismissal_restrictions: Option<Restrictions>,
    require_code_owner_reviews: bool,
    require_last_push_approval: Option<bool>,
    required_approving_review_count: u8,
}

#[derive(serde::Deserialize)]
struct Restrictions {
    apps: Option<Vec<App>>,
    teams: Vec<Team>,
    users: Vec<User>,
}

#[derive(serde::Deserialize)]
struct App {
    slug: String,
}

#[derive(serde::Deserialize)]
struct Team {
    slug: String,
}

#[derive(serde::Deserialize)]
struct User {
    login: String,
}

pub fn normalize(value: serde_json::Value) -> Result<types::Protection, Error> {
    let live: Live = serde_json::from_value(value)?;
    Ok(types::Protection {
        allow_deletions: live.allow_deletions.enabled,
        allow_force_pushes: live.allow_force_pushes.enabled,
        enforce_admins: live.enforce_admins.enabled,
        required_linear_history: live.required_linear_history.enabled,
        required_pull_request_reviews: live.required_pull_request_reviews.map(reviews),
        required_status_checks: live.required_status_checks,
        restrictions: live.restrictions.map(restrictions),
    })
}

fn reviews(live: ReviewRequirements) -> types::ReviewRequirements {
    types::ReviewRequirements {
        bypass_pull_request_allowances: live.bypass_pull_request_allowances.map(restrictions),
        dismiss_stale_reviews: live.dismiss_stale_reviews,
        dismissal_restrictions: live.dismissal_restrictions.map(restrictions),
        require_code_owner_reviews: live.require_code_owner_reviews,
        require_last_push_approval: live.require_last_push_approval,
        required_approving_review_count: live.required_approving_review_count,
    }
}

fn restrictions(live: Restrictions) -> types::Restrictions {
    types::Restrictions {
        apps: live
            .apps
            .map(|apps| apps.into_iter().map(|app| app.slug).collect()),
        teams: live.teams.into_iter().map(|team| team.slug).collect(),
        users: live.users.into_iter().map(|user| user.login).collect(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn github_response_becomes_an_update_payload() {
        let review_count: u8 = 1;
        let value = serde_json::json!({
            "url": "https://api.github.com/protection",
            "allow_deletions": {"enabled": false},
            "allow_force_pushes": {"enabled": true},
            "enforce_admins": {"enabled": true},
            "required_linear_history": {"enabled": true},
            "required_status_checks": null,
            "required_pull_request_reviews": {
                "url": "https://api.github.com/reviews",
                "dismiss_stale_reviews": true,
                "require_code_owner_reviews": true,
                "required_approving_review_count": review_count,
                "dismissal_restrictions": {"users": [{"login": "alice"}], "teams": [{"slug": "dev"}]}
            },
            "restrictions": {"users": [{"login": "alice"}], "teams": [], "apps": [{"slug": "bot"}]}
        });
        let protection = super::normalize(value).expect("valid response");
        let encoded = serde_json::to_value(protection).expect("serializes");
        assert_eq!(
            encoded.pointer("/restrictions/users"),
            Some(&serde_json::json!(["alice"]))
        );
        assert_eq!(
            encoded.pointer("/required_pull_request_reviews/dismissal_restrictions/teams"),
            Some(&serde_json::json!(["dev"]))
        );
        assert!(encoded.get("url").is_none());
    }
}
