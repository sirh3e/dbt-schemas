#!/bin/sh
# commit-msg hook: the first line must be a Conventional Commit, as
# git-cliff (cliff.toml) groups the changelog by it.
# https://www.conventionalcommits.org
set -eu
first=$(grep -v '^#' "$1" | head -n 1)
types='feat|fix|docs|perf|refactor|style|test|build|ci|chore|revert'
if printf '%s\n' "$first" | grep -Eq "^($types)(\([a-z0-9._/-]+\))?!?: .+" \
    || printf '%s\n' "$first" | grep -Eq '^(Merge|Revert|fixup!|squash!) '; then
    exit 0
fi
echo "error: not a Conventional Commit: $first" >&2
echo "  expected <type>(<scope>)?: <description>, <type> one of: $types" >&2
exit 1
