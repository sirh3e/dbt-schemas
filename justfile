# dbt-schemas — `just` lists the recipes.

default:
    @just --list

# ---------------------------------------------------------------- build

# Build the crate (build.rs preprocesses the schemas and generates the types)
[group('build')]
build:
    cargo build

# Run every test (the crate's, and the preprocessing rules')
[group('build')]
test:
    cargo test

# (the processed schemas, and the generated types formatted with rustfmt)
# Write what build.rs made into target/schemas/, to read
[group('build')]
generated:
    #!/usr/bin/env bash
    set -euo pipefail
    out=$(cargo build --message-format=json --quiet \
        | jq -r 'select(.reason == "build-script-executed" and (.package_id | contains("dbt-schemas"))) | .out_dir' \
        | tail -n 1)
    rm -rf target/schemas && mkdir -p target/schemas
    cp "$out"/*.json "$out"/*.rs target/schemas/
    rustfmt --edition 2024 target/schemas/*.rs
    ls target/schemas

# ------------------------------------------------------------------- ci

# Everything CI checks: formatting, lints, tests
[group('ci')]
check: fmt-check lint test

# Format the code
[group('ci')]
fmt:
    cargo fmt --all

# Fail on unformatted code
[group('ci')]
fmt-check:
    cargo fmt --all --check

# Clippy with zero warnings
[group('ci')]
lint:
    cargo clippy --all-targets -- -D warnings

# Install the git hooks (pre-commit and commit-msg) via prek
[group('ci')]
hooks:
    prek install

# Run every hook on every file
[group('ci')]
hooks-run:
    prek run --all-files

# -------------------------------------------------------------- release

# Regenerate CHANGELOG.md from the Conventional Commit history (git-cliff)
[group('release')]
changelog:
    git cliff -o CHANGELOG.md

# The changes since the last tag, as they would read in CHANGELOG.md
[group('release')]
changelog-unreleased:
    git cliff --unreleased

# The next version, as git-cliff works it out from the commits
[group('release')]
next-version:
    git cliff --bumped-version

# (guards, bump, check, changelog, commit; main takes pull requests only, so
# the commit travels as one, and after its merge `just release-tag` tags main)
# Prepare a release on a release/v<version> branch
[group('release')]
release-prep version:
    #!/usr/bin/env bash
    set -euo pipefail
    v="{{version}}"
    [ -z "$(git status --porcelain)" ] || { echo "error: working tree not clean" >&2; exit 1; }
    [ "$(git branch --show-current)" = "main" ] || { echo "error: start from main" >&2; exit 1; }
    git fetch origin main
    [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || { echo "error: main is not up to date with origin/main" >&2; exit 1; }
    git switch -c "release/v$v"
    perl -pi -e 'if (!$done && s/^version = ".*"/version = "'"$v"'"/) { $done = 1 }' Cargo.toml
    cargo update --workspace --quiet
    just check
    git cliff --tag "v$v" -o CHANGELOG.md
    git add Cargo.toml Cargo.lock CHANGELOG.md
    git commit -m "chore(release): v$v"
    echo "release/v$v ready — push it and open a pull request titled 'chore(release): v$v';"
    echo "after its merge, on updated main: just release-tag $v"

# (the push is left to you, as a tag is what consumers pin:
# `dbt-schemas = { git = …, tag = "v<version>" }`)
# Tag the merged release commit on main
[group('release')]
release-tag version:
    #!/usr/bin/env bash
    set -euo pipefail
    v="{{version}}"
    [ -z "$(git status --porcelain)" ] || { echo "error: working tree not clean" >&2; exit 1; }
    [ "$(git branch --show-current)" = "main" ] || { echo "error: not on main" >&2; exit 1; }
    git fetch origin main
    [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || { echo "error: main is not up to date with origin/main" >&2; exit 1; }
    grep -q "^version = \"$v\"" Cargo.toml || { echo "error: Cargo.toml is not at $v — merge the release pull request first" >&2; exit 1; }
    git tag -a "v$v" -m "dbt-schemas v$v"
    echo "tagged v$v — review, then publish with: git push origin v$v"

# ---------------------------------------------------------- maintenance

# Remove build artifacts
[group('maintenance')]
clean:
    cargo clean
