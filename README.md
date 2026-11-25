# dbt-schemas

Rust types for dbt's YAML files and JSON artifacts, generated from dbt Labs'
own JSON Schemas with [typify](https://github.com/oxidecomputer/typify), and
helpers that read a dbt YAML file once with [saphyr](https://github.com/saphyr-rs/saphyr)
and keep every node's position.

It began as the same code copied into two projects, a dbt language server and
a dbt planner, and is the one copy both read now.

Every variant and version dbt Labs publishes is here, each behind a Cargo
feature, from two upstream repositories checked out as git submodules:

| Upstream | Variants and versions | Module | Feature |
|---|---|---|---|
| [dbt-jsonschema](https://github.com/dbt-labs/dbt-jsonschema) | `1.5`, `1.6`, `1.7`, `latest`, `latest_fusion`: `dbt_project`, `dbt_yml_files`, `packages`, `selectors`, `dbt_cloud`, `dependencies`; Fusion also `catalogs`, `profiles` | `generated::yaml::<variant>::<file>` | `yaml-<variant>` (`yaml-1-6`, `yaml-latest-fusion`) |
| [schemas.getdbt.com](https://github.com/dbt-labs/schemas.getdbt.com) | `manifest` v0–v12, `run-results` v0–v6, `sources` v0–v3, `catalog` v0–v1, `freshness` v0 | `artifacts::<family>::v<N>` | `<family>-v<N>` (`run-results-v5`) |
| | each manifest's projection (below) | `artifacts::manifest::projection::v<N>` | `manifest-projection-v<N>` |

- **The default features** are what the newest dbt release reads,
  `dbt-latest` (below): today the latest YAML schemas, run-results v6, sources
  v3, catalog v1 and the manifest v12 projection, what the crate has always
  built. A version upstream adds is an optional feature until its dbt release
  is in the table. `yaml-all`, `artifacts-all`, `manifests-full` (every manifest's
  complete types, large) and `all` group the rest; `Cargo.toml` lists them.
- **A family's root is its newest version the build has:** `artifacts::run_results::*` is
  `run_results::v6::*`, `artifacts::manifest::*` the newest projection, and
  `generated::dbt_project` (and `dbt_yml_files`, `packages`, `selectors`) the
  latest YAML schemas, where they have always been.
- **Reading any version:** `artifacts::AnyRunResults::from_value(json)` (and
  `AnySources`, `AnyCatalog`, `AnyFreshness`, `AnyManifest`,
  `AnyManifestProjection`) reads a document as the version its
  `metadata.dbt_schema_version` names, with a variant per version the build
  has; `artifacts::sniff` says which schema a document names, and
  `artifacts::SCHEMA_IDS` lists every one upstream has.

## Feature flags by dbt release

Most projects know their dbt version, not their schema numbers, so there is a
feature per dbt release, which turns on what that release writes and reads (a
manifest as its projection; add `manifest-v<N>` for its complete types):

```toml
dbt-schemas = { git = "https://github.com/sirh3e/dbt-schemas", tag = "v0.1.1", default-features = false, features = ["dbt-1-7"] }
```

| Feature | dbt | manifest | run-results | sources | catalog | freshness | YAML |
|---|---|---|---|---|---|---|---|
| `dbt-0-19` | 0.19 | v1 | v1 | v1 | v1 | | |
| `dbt-0-20` | 0.20 | v2 | v2 | v1 | v1 | | |
| `dbt-0-21` | 0.21 | v3 | v3 | v2 | v1 | | |
| `dbt-1-0` | 1.0 | v4 | v4 | v3 | v1 | | |
| `dbt-1-1` … `dbt-1-4` | 1.1 … 1.4 | v5 … v8 | v4 | v3 | v1 | | |
| `dbt-1-5` | 1.5 | v9 | v4 | v3 | v1 | | `1.5` |
| `dbt-1-6` | 1.6 | v10 | v4 | v3 | v1 | | `1.6` |
| `dbt-1-7` | 1.7 | v11 | v5 | v3 | v1 | | `1.7` |
| `dbt-1-8` … `dbt-1-11` | 1.8 … 1.11 | v12 | v6 | v3 | v1 | | `latest` |
| `dbt-2-0` | 2.0 (Fusion) | v12 | v6 | v3 | v1 | v0 | `latest_fusion` |

- `dbt-latest` is `dbt-1-11`, which the default features equal, and `dbt-fusion` is `dbt-2-0`.
- `dbt-0`, `dbt-1` and `dbt-2` turn on every release of a major version, and `dbt-all` all of them.
- **Where the table comes from:** every 0.x and 1.x row was read from dbt-core's own source at the release's tag, and the 2.0 row from dbt's documentation. Rows are kept by hand in `build/dbt_releases.rs`, which says where each one comes from, and a test holds each row to the schemas upstream has.
- **At run time,** `dbt::release("1.7.14")` gives a release's schemas and feature, and whether this build has them. `dbt::releases_of("manifest", 12)` gives the releases that write a schema version, and `artifacts::dbt_releases(&json)` the releases a document can be from.
- **A schema version upstream adds is released at once** with its own feature. The bot's pull request lists it under "Not in any dbt release flag yet" until someone adds the release's row.

| Module | What |
|---|---|
| `yaml` | `load_marked`, `to_json`, `from_json`, `get`, `as_str`, `entries`, `items`, `position_of`, `span_of`, `syntax_error`, `Position`, `Span` |
| `schema_keys`, `schema_walk` | the property names and values the latest schemas allow, at the top or along any YAML path, for completion |
| `avro` | (feature `avro`) every schema built as an Avro schema, `encode` and `decode` |
| `processed` | the processed schemas as text (`processed::ALL`, `processed::schema("yaml/v1_6/dbt_project")`) and the latest YAML ones parsed |

## Usage

Add it, pinned by a release's tag; the default features read what the newest
dbt release writes (see the feature table above for another release):

```toml
[dependencies]
dbt-schemas = { git = "https://github.com/sirh3e/dbt-schemas", tag = "v0.1.1" }
serde_json = "1"
```

Each example below is a runnable program in [`examples/`](examples/), built by
CI with the rest; run one with `cargo run --example <name>`, with a path or
words after `--` or on its built-in sample. They are written as the crate is:
small pure functions composed into a result, with the files and the printing
left to `main`.

| Example | Shows |
|---|---|
| [`read_project`](examples/read_project.rs) | a `dbt_project.yml` into `DbtProject`, and where each key is written |
| [`run_results`](examples/run_results.rs) | a `run_results.json` of any version, counted by status, and the dbt releases it can be from |
| [`dbt_release`](examples/dbt_release.rs) | what a dbt version writes and the feature that builds it, and which releases write a schema |
| [`complete`](examples/complete.rs) | the keys a properties file allows at a YAML path, for an editor's completion |

**A YAML file, typed and with positions.** Parse once; the same tree gives
the value and where each node is:

```rust
use std::path::Path;
use dbt_schemas::{generated::dbt_project::DbtProject, yaml};

fn project_name(text: &str, path: &Path) -> dbt_schemas::Result<(String, dbt_schemas::Position)> {
    let marked = yaml::load_marked(text, path)?;
    let project: DbtProject = yaml::from_json(yaml::to_json(&marked), path)?;
    let at = yaml::get(&marked, "name").map(yaml::position_of).unwrap_or_default();
    Ok((project.name, at))
}
```

A real project's `config:` blocks often break the strict YAML types (see
below): prune the value from `to_json` to the keys you read before `from_json`.

**An artifact of whichever version wrote it.** `Any<Family>::from_value`
reads a document as the schema its `metadata.dbt_schema_version` names; the
enum is `#[non_exhaustive]` and has a variant for each version the build has,
so keep a last arm for the others:

```rust
use dbt_schemas::artifacts::AnyRunResults;

fn failed(value: serde_json::Value) -> Result<Vec<String>, String> {
    match AnyRunResults::from_value(value).map_err(|e| e.to_string())? {
        AnyRunResults::V6(run) => Ok(run
            .results
            .into_iter()
            .filter(|r| r.status.as_deref() == Some("fail"))
            .filter_map(|r| r.unique_id)
            .collect()),
        other => Err(format!("run results v{} are not read here", other.version())),
    }
}
```

Every field of an artifact type is an `Option` and unknown fields pass, so
one type reads the files of many dbt versions. `artifacts::sniff(&value)`
says which schema a document names without reading it, and
`artifacts::dbt_releases(&value)` which dbt releases it can be from.

**A dbt release's schemas.**

```rust
let release = dbt_schemas::dbt::release("1.7.14").expect("a known release");
assert_eq!(release.schema("manifest"), Some(11));
assert_eq!(release.feature, "dbt-1-7"); // turn it on to read what 1.7 writes
let built = release.built;              // whether this build has those schemas
```

**Completion along a YAML path.**

```rust
use dbt_schemas::schema_walk::{properties_keys_at, properties_values_at, Step};

let column_keys = properties_keys_at(&[Step::Key("models"), Step::Item, Step::Key("columns"), Step::Item]);
let access = properties_values_at(&[Step::Key("models"), Step::Item, Step::Key("access")]);
```

## Avro (feature `avro`)

Opt in with the `avro` feature, which is in no group and not in the
defaults; it adds [apache-avro](https://crates.io/crates/apache-avro) 0.21.
`build.rs` then writes an Avro schema (`.avsc`) beside each JSON Schema the
other features build, the YAML ones included, and `avro` encodes documents
with them:

```toml
dbt-schemas = { git = "https://github.com/sirh3e/dbt-schemas", tag = "…", features = ["avro"] }
```

```rust
use dbt_schemas::avro;

let schema = avro::schema_of(&run_results)?;     // run_results/v6, by its dbt_schema_version
let bytes = schema.encode(&run_results)?;        // one Avro datum
let back = schema.decode(&bytes)?;               // the document, as far as the schema knows it
let writer = apache_avro::Writer::new(schema.apache(), Vec::new()); // a container file
```

- **Which schema:** `avro::schema("run_results/v6")` by module path, as
  `processed::schema` takes it, or `avro::schema_of(&json)` by the document's
  `metadata.dbt_schema_version` (a manifest as its projection; the complete
  one is `avro::schema("manifest/v12")`). `avro::ALL` lists every `.avsc`.
- **The mapping** is `build/avro.rs`, one rule per JSON Schema keyword: an
  object with properties is a record, one without is a map, an optional field
  is `["null", T]` with a `null` default, an enum is a string, a `$ref` is a
  named record, a union is a union. What Avro cannot hold, and what the schema
  leaves open (`config`, `adapter_response`), is **any JSON**: a string of the
  value's JSON text, marked `"dbt_schemas.json": true`. A key Avro refuses as
  a name (`post-hook`) is `post_hook`, its JSON name kept in `"dbt_schemas.name"`.
- **What is lost:** unknown fields, which an Avro record has no room for (the
  generated types let them pass); the difference between a `null` and an
  absent field; and whether a whole `double` was written `3` or `3.0`. A
  document reads back as the generated types read it; `tests/avro.rs` holds
  the two to each other.
- A document is walked against its schema, not through serde, so the values
  are the JSON's own. Maps have no order in Avro, so the bytes of two
  encodings of one document may differ.

## What the types are, and are not

They mirror the upstream formats and nothing else: map them into your own
types as soon as they are read.

- **The YAML types are strict.** dbt's unions of configs are written
  narrowly upstream, and real projects do not keep to them, so a whole file
  can be refused for one `config:` block. Prune the value from `to_json` to
  the keys you read before `from_json`, as both consumers do.
- **The artifact types are lenient.** dbt writes fields across versions that
  the official schemas forbid, so the artifact rules of `build/preprocess.rs` make every
  field optional, lets unknown fields pass, reads every status as a string
  and some fields (`stats`, `execution_time`, `generated_at` of a catalog) as
  any JSON value. The `$id` constants in `artifacts` are for tracing which
  dbt wrote a file, never for refusing it.
- **The derives.** Every type derives `Debug`, `Clone`, `PartialEq` and serde's
  two, `Default` where every field has one, and `Eq` and `Hash` wherever no
  float is inside it, at any depth (`build/derives.rs`): so a document can be
  compared, and most of its parts used as keys. Maps are `BTreeMap`, which has
  both and serializes in key order. A float upstream adds to a schema takes
  `Eq` and `Hash` from every type that reaches it, which a release then says
  is breaking.
- **The manifest is a projection.** Typifying the 946 KB schema would cost
  thousands of types; `--mode=manifest-projection` the projection emits the fields
  consumers read (node identity, code, checksum, dependencies, `relation_name`,
  `config` and `columns` as any JSON value; sources; macros; `parent_map`, `child_map`;
  metadata), and fails if any of them no longer appears upstream. A field
  you need is a line in the `projection_*` tables of `build/preprocess.rs`.

## How the types are made

    schemas/upstream/dbt-jsonschema/        git submodule of dbt-labs/dbt-jsonschema
    schemas/upstream/schemas.getdbt.com/    git submodule of dbt-labs/schemas.getdbt.com
    build/catalog.rs      every schema the submodules hold, as a module and a feature
    build/preprocess.rs   the rules that make a schema typify-digestible, each with why
    build/derives.rs      which types derive Eq and Hash: those with no float inside
    build/avro.rs         a processed schema as an Avro schema (feature `avro`)
    build.rs              preprocesses each schema whose feature is on and generates its
                          types with typify into OUT_DIR, at build time

Nothing generated is committed, and a new upstream version needs no code: it
becomes a module and a feature by itself (`just features` writes the feature).
`just generated` writes what `build.rs` made into `target/schemas/` to read. A
name the manifest projection or `RELAX_TO_ANY` relies on that is gone from the
newest version fails the build, so upstream cannot drift silently. A schema
typify cannot read fails the build too, until it is fixed or listed in
`UNSUPPORTED` of `build.rs` with why; none is listed today.

## Following upstream

`.github/workflows/upstream.yml` runs every day at 05:17 UTC, or on demand. When
dbt Labs changed either repository it moves the submodule, builds and tests
every variant and version, lets
[cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks) pick the
version against the last release (more than a patch bumps the minor while the
crate is 0.x), commits the sync and its release and opens a pull request that
merges itself once CI passes. `release.yml` then tags `v<version>` and publishes a
GitHub Release with the changelog's section. A change that does not build stops
there, for a person to look at. `just sync` does the same on a machine
(`scripts/sync-upstream.sh`).

**Setting it up, once:**

1. In the repository's settings, allow auto-merge, and protect `main` with the
   status check `check` required.
2. Create a fine-grained personal access token (or a GitHub App) for this
   repository with *Contents* and *Pull requests* read and write, and store it as
   the Actions secret `DBT_SCHEMAS_BOT_TOKEN`. The bot needs it because a pull
   request opened with `GITHUB_TOKEN` starts no CI, and so could never merge.

Consumers pin a release by tag (`dbt-schemas = { git = "https://github.com/sirh3e/dbt-schemas", tag = "v0.1.1" }`);
Cargo checks the submodules out with it.

## Working on it

    git clone --recurse-submodules https://github.com/sirh3e/dbt-schemas
    just              the recipes
    just check        rustfmt, clippy and the tests of the default features
    just check-all    what CI runs: also every feature, none, and each one alone
    just hooks        install the git hooks (prek, or pre-commit; shellcheck on PATH): rustfmt, clippy,
                      tests and shellcheck on commit, and a Conventional Commit check
                      of the message, which git-cliff groups CHANGELOG.md by
    just features     write Cargo.toml's [features] again from the submodules
    just sync         follow upstream as the bot does
    just changelog    regenerate CHANGELOG.md

## Licence

The code is MIT ([LICENSE](LICENSE)). The vendored schemas and what is
derived from them are dbt Labs', Apache-2.0 ([LICENSE-APACHE](LICENSE-APACHE)).
