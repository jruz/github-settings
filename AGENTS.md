# Agent instructions

`github-settings` is a Rust CLI for managing GitHub repository settings through `github-settings.json`. It pulls live settings, previews changes with a read-only plan, applies changes through the GitHub REST API, and exports the declaration's JSON Schema.

## Architecture

- `src/main.rs`: async entrypoint and channel-based output printing.
- `src/cli.rs`: command parsing.
- `src/app.rs`: command dispatch.
- `src/commands/`: one thin module per command.
- `src/commands/version.rs`: version metadata from the release skill.
- `src/core/`: business logic.
- `src/core/session.rs`: API client setup and load-and-compare workflow.
- `src/core/plan.rs`: change calculation.
- `src/core/push.rs`: change application and visibility polling.
- `src/core/output.rs`: output events and formatting.
- `src/libs/`: self-contained units that could be extracted into their own crate.
- `src/libs/config.rs`: declaration file access.
- `src/libs/env.rs`: credential validation.
- `src/libs/git.rs`: repository resolution.
- `src/libs/github/`: GitHub API access.
- `src/utils/`: small generic helpers.
- `src/utils/types.rs`: declaration types and validation.
- `build.rs`: Git revision and build timestamp with typed errors using `thiserror`.
- `tests/commands/`: one binary test module per command.
- `tests/utils/mock.rs`: mock GitHub API, reached through `GITHUB_API_URL`.

## Declaration keys

- Name keys in `github-settings.json` after the GitHub REST API field they manage.
- Where the API has no JSON field, name the key after the endpoint path segment.
- Use `_` in keys, never `-`.
- Keep values in GitHub's format, including `-` in topics, labels and branch names.

## Errors

- Show users a short message in declaration terms with a `help` hint.
- Record request and failure details with `tracing::error!` at the point of failure.
- Failures are appended as JSON lines to `$XDG_STATE_HOME/github-settings/errors.jsonl`, or `~/.local/state/github-settings/errors.jsonl` when `XDG_STATE_HOME` is unset.
- Run `just logs-show` to read the log and `just logs-clean` to delete it.

## Layers

- `main.rs` calls `commands` and `utils`.
- `commands` calls `core` and `utils`.
- `core` calls `libs` and `utils`.
- `libs` calls `utils`.
- `utils` calls nothing.
- `just rust::architecture-check` enforces these rules with `scripts/architecture-check.nu`.

## Development

- Use `justfile` and `.just/` recipes for development tasks.
- Run Cargo commands through `sandbox`.
- Run `just check`, `just test`, and `slop-detector` after changes.
- Run `just schema-build` after changing declaration types.

## Configuration tools

- Run `just tools::check` after configuration changes.
- Run `just tools::fix` to format configuration.
- `.tools/` provides its own Nix shell through `.just/tools.justfile`.
- Keep these checks separate from `just check` and CI.

## Release

- `just release bump <version>` updates Cargo manifests and commits the version bump.
- Push the bump to `main`; CI handles tagging and publishing.
- `just release delete <tag>` deletes a published release and its tag.
