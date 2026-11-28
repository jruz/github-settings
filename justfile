mod rust '.just/rust.justfile'
mod just '.just/just.justfile'
mod oxfmt '.just/oxfmt.justfile'
mod git '.just/git.justfile'
mod tools '.just/tools.justfile'

check:
    just just::fmt-check
    just oxfmt::fmt-check
    just rust::check

fix:
    just just::fmt-fix
    just oxfmt::fmt-fix
    just rust::fix

setup:
    direnv allow
    just git::hooks-setup

install:
    just rust::install

test:
    just rust::test

run *ARGS='':
    just rust::run {{ ARGS }}

[positional-arguments]
release +ARGS:
    nu scripts/release.nu "$@"

schema-build:
    sandbox cargo run -- schema > github-settings.schema.json

logs-show:
    #!/usr/bin/env nu
    let state = (
        $env.XDG_STATE_HOME?
        | default --empty ($env.HOME | path join .local state)
    )
    open --raw ($state | path join github-settings errors.jsonl)
        | from json --objects
        | each {|entry|
            {
                timestamp: $entry.timestamp
                process: $entry.span?.process?
                target: $entry.target
            } | merge $entry.fields
        }
        | to yaml

logs-clean:
    #!/usr/bin/env nu
    let state = (
        $env.XDG_STATE_HOME?
        | default --empty ($env.HOME | path join .local state)
    )
    rm --force ($state | path join github-settings errors.jsonl)
