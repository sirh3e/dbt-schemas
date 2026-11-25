#!/usr/bin/env bash
# Move the upstream submodules to their newest commits and prepare a release
# of what changed: the bot's job (.github/workflows/upstream.yml), and
# `just sync` on a machine.
#
#   1. git submodule update --remote; stop when nothing moved
#   2. write Cargo.toml's [features] again (a new schema is a new feature)
#   3. build and test every feature: a schema typify cannot read, or a name
#      the drift guards hold gone upstream, fails here
#   4. cargo-semver-checks against the last tag picks the version: what it
#      finds more than a patch bumps the minor (0.x), anything else the patch
#   5. commit the sync (feat: a new schema file; fix: a changed one; `!` when
#      breaking), then `chore(release): v<version>` with CHANGELOG.md; when no
#      schema file changed, only the sync, as `chore(schemas)`: no release
#
# Writes the pull request's body to target/sync/pr-body.md and, on GitHub, `changed`, `version` and `title` to
# $GITHUB_OUTPUT. Leaves the commits on the current branch; pushing and the
# pull request are the workflow's.
set -euo pipefail
cd "$(dirname "$0")/.."

output() {
    if [ -n "${GITHUB_OUTPUT:-}" ]; then echo "$1=$2" >> "$GITHUB_OUTPUT"; fi
}

[ -z "$(git status --porcelain)" ] || { echo "error: the working tree is not clean" >&2; exit 1; }

declare -A old new
subs=$(git config -f .gitmodules --get-regexp '^submodule\..*\.path$' | awk '{print $2}')
for sub in $subs; do
    old[$sub]=$(git -C "$sub" rev-parse HEAD)
done
git submodule update --init --remote
for sub in $subs; do
    new[$sub]=$(git -C "$sub" rev-parse HEAD)
done

moved=()
for sub in $subs; do
    if [ "${old[$sub]}" != "${new[$sub]}" ]; then moved+=("$sub"); fi
done
if [ ${#moved[@]} -eq 0 ]; then
    echo "upstream has not moved"
    output changed false
    exit 0
fi

# the history of each moved submodule, and which schema files it touched
mkdir -p target/sync
body=target/sync/pr-body.md
added=()
changed=()
{
    echo "Upstream moved; this pull request follows it and releases the result."
    echo
    for sub in "${moved[@]}"; do
        url=$(git config -f .gitmodules --get "submodule.$sub.url")
        echo "### \`$sub\`: ${old[$sub]:0:10} → ${new[$sub]:0:10}"
        echo
        # a shallow clone may lack the old commit's ancestry; ask for it
        git -C "$sub" fetch --quiet --unshallow 2>/dev/null || true
        git -C "$sub" log --format="- [\`%h\`]($url/commit/%H) %s (%as)" "${old[$sub]}..${new[$sub]}" || true
        echo
        while read -r status path; do
            case "$path" in
                *.json) ;;
                *) continue ;;
            esac
            case "$status" in
                A*) added+=("$sub/$path") ;;
                *) changed+=("$sub/$path") ;;
            esac
        done < <(git -C "$sub" diff --name-status "${old[$sub]}" "${new[$sub]}")
    done
    if [ ${#added[@]} -gt 0 ]; then
        echo "New schema files:"; for f in "${added[@]}"; do echo "- \`$f\`"; done; echo
    fi
    if [ ${#changed[@]} -gt 0 ]; then
        echo "Changed or removed schema files:"; for f in "${changed[@]}"; do echo "- \`$f\`"; done; echo
    fi
} > "$body"

names=$(printf '%s\n' "${moved[@]}" | xargs -n1 basename | paste -sd, - | sed 's/,/, /g')
sync_message() {
    echo "$1"
    echo
    for sub in "${moved[@]}"; do echo "$sub: ${old[$sub]} -> ${new[$sub]}"; done
}

# upstream moved, but no schema did (a README, the site's HTML): follow it
# without a release, as the types are the same
if [ ${#added[@]} -eq 0 ] && [ ${#changed[@]} -eq 0 ]; then
    title="chore(schemas): follow upstream $names"
    {
        echo "### Release"
        echo
        echo "No schema file changed, so the types are the same: no release."
    } >> "$body"
    git add -- .gitmodules schemas
    sync_message "$title" | git commit --quiet -F -
    echo "followed upstream without a release: $title"
    output changed true
    output version ""
    output title "$title"
    exit 0
fi

DBT_SCHEMAS_WRITE=1 cargo test --quiet --test features
just check
cargo test --quiet --all-features

# what no dbt release flag includes yet: each family's newest version and any
# YAML variant. Not a failure: it is built and released with its own feature,
# and a person adds its dbt release to build/dbt_releases.rs.
features_table=$(awk '/^\[features\]/ { on = 1; next } /^\[/ { on = 0 } on' Cargo.toml)
in_dbt_flags=$(printf '%s\n' "$features_table" | awk '
    /^[a-z0-9-]+ = / { key = $1 }
    key ~ /^dbt-/ { line = $0; while (match(line, /"[^"]+"/)) { print substr(line, RSTART + 1, RLENGTH - 2); line = substr(line, RSTART + RLENGTH) } }
' | sort -u)
leaves=$(printf '%s\n' "$features_table" | sed -n 's/^\([a-z0-9-]*\) = \[\]$/\1/p')
newest=$( { printf '%s\n' "$leaves" | grep -E -- '-v[0-9]+$' | grep -v '^manifest-v' \
    | sed -E 's/^(.*)-v([0-9]+)$/\1 \2/' | sort -k1,1 -k2,2n \
    | awk '{ last[$1] = $2 } END { for (f in last) print f "-v" last[f] }'
    printf '%s\n' "$leaves" | grep '^yaml-'; } | sort)
unmapped=()
while read -r feature; do
    [ -n "$feature" ] || continue
    if ! printf '%s\n' "$in_dbt_flags" | grep -qx -- "$feature"; then unmapped+=("$feature"); fi
done <<< "$newest"
if [ ${#unmapped[@]} -gt 0 ]; then
    {
        echo "### Not in any dbt release flag yet"
        echo
        echo "These are built and released with their own feature, but no \`dbt-<major>-<minor>\`"
        echo "feature includes them. Add the dbt release that writes them to"
        echo "\`build/dbt_releases.rs\` (and \`just features\`):"
        echo
        for f in "${unmapped[@]}"; do echo "- \`$f\`"; done
        echo
    } >> "$body"
fi

# the version, by what the public types did since the last release
last=$(git describe --tags --abbrev=0 --match 'v*')
current=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
baseline=$(mktemp -d)
git worktree add --quiet --detach "$baseline/crate" "$last"
git -C "$baseline/crate" submodule update --quiet --init
# the level cargo-semver-checks asks for, when it asks for more than a patch
needs='patch'
if ! cargo semver-checks --all-features --release-type patch \
    --baseline-root "$baseline/crate" > "$baseline/semver.txt" 2>&1; then
    needs=$(sed -n 's/.*semver requires new \(major\|minor\) version.*/\1/p' "$baseline/semver.txt" | head -n 1)
    if [ -z "$needs" ]; then
        cat "$baseline/semver.txt" >&2
        git worktree remove --force "$baseline/crate"
        exit 1
    fi
fi
git worktree remove --force "$baseline/crate"

# 0.x: whatever is more than a patch bumps the minor, as Cargo reads 0.x
IFS=. read -r major minor patch <<< "$current"
breaking=false
case "$needs:$major" in
    patch:*) version="$major.$minor.$((patch + 1))" ;;
    *:0) version="0.$((minor + 1)).0"; breaking=true ;;
    major:*) version="$((major + 1)).0.0"; breaking=true ;;
    minor:*) version="$major.$((minor + 1)).0" ;;
esac

if [ "$breaking" = true ]; then
    kind='feat(schemas)!'
elif [ ${#added[@]} -gt 0 ]; then
    kind='feat(schemas)'
else
    kind='fix(schemas)'
fi
title="$kind: follow upstream $names"
# what is new, in the words of the features: `run-results v7`, `yaml 1.8/dbt_project`
if [ ${#added[@]} -gt 0 ]; then
    new_names=$(for f in "${added[@]}"; do
        case "$f" in
            */dbt/*/v*.json) f=${f#*/dbt/}; echo "${f%%/*} $(basename "${f%.json}")" ;;
            */schemas/*/*.json) f=${f#*/schemas/}; echo "yaml ${f%%-*}" ;;
        esac
    done | sort -u | paste -sd, - | sed 's/,/, /g')
    if [ -n "$new_names" ]; then title="$title: $new_names"; fi
fi
{
    echo
    echo "### Release"
    echo
    if [ "$breaking" = true ]; then
        echo "cargo-semver-checks asks for a new $needs version since $last, so this is **v$version**:"
        echo
        echo '```'
        sed -n '1,80p' "$baseline/semver.txt"
        echo '```'
    else
        echo "cargo-semver-checks found no breaking change since $last, so this is **v$version**."
    fi
} >> "$body"
rm -rf "$baseline"

git add -A -- .gitmodules schemas Cargo.toml Cargo.lock
sync_message "$title" | git commit --quiet -F -

perl -pi -e 'if (!$done && s/^version = ".*"/version = "'"$version"'"/) { $done = 1 }' Cargo.toml
cargo update --quiet --workspace
git cliff --tag "v$version" -o CHANGELOG.md
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit --quiet -m "chore(release): v$version"

echo "prepared v$version: $title"
output changed true
output version "$version"
output title "$title"
