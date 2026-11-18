use crate::commands::version;

#[derive(Debug, clap::Parser)]
#[command(
    version = version::text(),
    about = "Pull, plan, and push declarative GitHub repository settings"
)]
pub struct Arguments {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    #[command(about = "Preview changes from the declaration without modifying GitHub")]
    Plan,
    #[command(about = "Write live GitHub settings to the declaration file")]
    Pull {
        #[arg(long, help = "Overwrite an existing declaration")]
        force: bool,
    },
    #[command(about = "Apply the declaration to GitHub")]
    Push,
    #[command(about = "Print the declaration JSON Schema")]
    Schema,
    #[command(about = "Show version and build metadata")]
    Version,
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    #[test]
    fn file_override_is_rejected() {
        for command in ["pull", "plan", "push", "schema"] {
            super::Arguments::try_parse_from(["github-settings", command, "--file", "other.json"])
                .expect_err("file override is not supported");
            super::Arguments::try_parse_from(["github-settings", "--file", "other.json", command])
                .expect_err("global file override is not supported");
        }
    }

    #[test]
    fn commands_are_parsed() {
        let pull = super::Arguments::try_parse_from(["github-settings", "pull", "--force"])
            .expect("valid pull");
        assert!(matches!(pull.command, super::Command::Pull { force: true }));
        let plan =
            super::Arguments::try_parse_from(["github-settings", "plan"]).expect("valid plan");
        assert!(matches!(plan.command, super::Command::Plan));
        super::Arguments::try_parse_from(["github-settings", "apply"])
            .expect_err("apply is not supported");
        for command in ["pull", "plan", "push"] {
            super::Arguments::try_parse_from(["github-settings", command, "--repo", "owner/repo"])
                .expect_err("repository override is not supported");
        }
        super::Arguments::try_parse_from(["github-settings", "push", "--force"])
            .expect_err("force is only for pull");
    }
}
