use std::path;

use super::plan;
use crate::{
    libs::{
        config, env, git,
        github::{api, live},
    },
    utils::types,
};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Api(#[from] api::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Environment(#[from] env::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Git(#[from] git::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Live(#[from] live::Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Plan(#[from] plan::Error),
}

pub struct Comparison {
    pub changes: Vec<plan::Change>,
    pub desired: types::Declaration,
}

pub fn client() -> Result<api::Client, Error> {
    let token = env::token()?;
    let base = env::api_url()?;
    let client = api::client(&token, &base)?;
    Ok(client)
}

#[tracing::instrument(skip_all)]
pub async fn read_changes(client: &api::Client) -> Result<Comparison, Error> {
    let repository = git::repository().await?;
    let file = path::Path::new("github-settings.json");
    let desired = config::read(file).await?;
    git::check_repository(&desired.repository, &repository)?;
    let branch = desired.settings.default_branch.as_str();
    let branch = Some(branch);
    let current = live::read(client, &desired.repository, branch).await?;
    let changes = plan::changes(&desired, &current)?;
    Ok(Comparison { changes, desired })
}
