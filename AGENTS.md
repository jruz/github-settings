# Agent instructions

`github-settings` is a Rust CLI for managing GitHub repository settings through `github-settings.json`. It pulls live settings, previews changes with a read-only plan, applies changes through the GitHub REST API, and exports the declaration's JSON Schema.

## Architecture

- `src/main.rs`: async entrypoint and channel-based output printing.
- `src/cli.rs`: command parsing and version metadata.
- `src/app.rs`: command dispatch.
- `src/commands/`: one module per command.
- `src/commands/shared.rs`: API client setup and load-and-compare workflow.
- `src/lib/`: supporting modules exposed internally as `library`.
- `src/lib/config.rs`: declaration file access.
- `src/lib/env.rs`: credential validation.
- `src/lib/git.rs`: repository resolution.
- `src/lib/types.rs`: declaration types and validation.
- `src/lib/plan.rs`: change calculation.
- `src/lib/github/`: GitHub API access.
- `src/lib/output.rs`: output events and formatting.

## Development

- Use `justfile` and `.just/` recipes for development tasks.
- Run Cargo commands through `sandbox`.
- Run `just check`, `just test`, and `slop-detector` after changes.
