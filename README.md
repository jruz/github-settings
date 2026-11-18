# github-settings

Manage GitHub repository settings through a JSON declaration.

- Requires `GITHUB_TOKEN` with permission to read and administer the repository.
- Calls the GitHub REST API directly.
- Does not require `gh`.

```sh
github-settings pull
github-settings plan
github-settings push
```

- `pull --force` overwrites an existing declaration.
- `plan` previews changes without writes.
- `push` applies changes to the current repository.
- `plan` and `push` reject declarations targeting a different repository.

- `just run` runs the app.
- `just check` runs linters.
- `just test` runs tests.
- `just install` installs the binary.

## Nix installation

- `nix profile add github:jruz/github-settings` installs the latest release binary.
- `nix profile add github:jruz/github-settings#source` builds from source.

## Licence

Licensed under the [EUPL 1.2](LICENSE).
