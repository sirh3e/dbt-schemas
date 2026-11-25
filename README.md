# dbt-schemas

Rust types for dbt's YAML files and JSON artifacts, generated from dbt Labs'
own JSON Schemas with [typify](https://github.com/oxidecomputer/typify), and
helpers that read a dbt YAML file once with [saphyr](https://github.com/saphyr-rs/saphyr)
and keep every node's position.

It began as the same code copied into two projects, a dbt language server and
a dbt planner, and is the one copy both read now.

| Module | What |
|---|---|
| `generated::dbt_project` | `dbt_project.yml` |
| `generated::dbt_yml_files` | properties files (`models/schema.yml`: models, sources, …) |
| `generated::packages`, `generated::selectors` | `packages.yml`, `selectors.yml` |
| `artifacts::{run_results, sources, catalog}` | `run_results.json` v6, `sources.json` v3, `catalog.json` v1 |
| `artifacts::manifest` | a projection of `manifest.json` v12 (below) |
| `yaml` | `load_marked`, `to_json`, `from_json`, `get`, `as_str`, `entries`, `items`, `position_of`, `span_of`, `syntax_error`, `Position`, `Span` |
| `schema_keys`, `schema_walk` | the property names and values the schemas allow, at the top or along any YAML path, for completion |
| `processed` | the processed YAML schemas as text and parsed |

```rust
use std::path::Path;
use dbt_schemas::{generated::dbt_project::DbtProject, yaml};

let path = Path::new("dbt_project.yml");
let marked = yaml::load_marked("name: shop\nmodel-paths: [models]\n", path)?;
let project: DbtProject = yaml::from_json(yaml::to_json(&marked), path)?;
let name_at = yaml::position_of(yaml::get(&marked, "name").unwrap());
# Ok::<_, dbt_schemas::Error>(())
```

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
- **The manifest is a projection.** Typifying the 946 KB schema would cost
  thousands of types; `--mode=manifest-projection` the projection emits the fields
  consumers read (node identity, code, checksum, dependencies, `relation_name`,
  `config` and `columns` as any JSON value; sources; macros; `parent_map`, `child_map`;
  metadata), and fails if any of them no longer appears upstream. A field
  you need is a line in the `projection_*` tables of `build/preprocess.rs`.

## How the types are made

    schemas/yaml/*-latest.json     pristine copies of dbt-jsonschema (never edited)
    schemas/artifacts/*-v*.json    pristine copies of schemas.getdbt.com (never edited)
    build/preprocess.rs            the rules that make them typify-digestible, each
                                   documented with why it exists
    build.rs                       preprocesses every schema and generates its types
                                   with typify into OUT_DIR, at build time

Nothing generated is committed. `just generated` writes what `build.rs` made
into `target/schemas/` to read: the processed schemas and the formatted types.
A name the manifest projection or `RELAX_TO_ANY` relies on that is gone
upstream fails the build, so updating a pristine copy cannot drift silently.
The upstream commits are in [NOTICE](NOTICE).

## Working on it

    just            the recipes
    just check      what CI runs: rustfmt, clippy, tests
    just hooks      install the git hooks (prek, or pre-commit): rustfmt,
                    clippy and tests on commit, and a Conventional Commit check
                    of the message, which git-cliff groups CHANGELOG.md by
    just changelog  regenerate CHANGELOG.md
    just release-prep 0.2.0, then after its merge: just release-tag 0.2.0

## Licence

The code is MIT ([LICENSE](LICENSE)). The vendored schemas and what is
derived from them are dbt Labs', Apache-2.0 ([LICENSE-APACHE](LICENSE-APACHE)).
