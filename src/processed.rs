//! The processed YAML schemas themselves (what `build.rs` made of the
//! pristine ones, and the generated types are compiled from), as text and
//! parsed, for what
//! reads a schema rather than a file: completion, highlighting, config keys.

use std::sync::LazyLock;

use serde_json::Value;

/// The processed `dbt_project.yml` schema.
pub const DBT_PROJECT_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/dbt_project.json"));
/// The processed properties files' schema.
pub const DBT_YML_FILES_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/dbt_yml_files.json"));
/// The processed `packages.yml` schema.
pub const PACKAGES_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/packages.json"));
/// The processed `selectors.yml` schema.
pub const SELECTORS_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/selectors.json"));

static DBT_PROJECT: LazyLock<Value> = LazyLock::new(|| parse(DBT_PROJECT_JSON));
static DBT_YML_FILES: LazyLock<Value> = LazyLock::new(|| parse(DBT_YML_FILES_JSON));

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("a bundled schema is valid json")
}

/// The `dbt_project.yml` schema, parsed once.
pub fn dbt_project() -> &'static Value {
    &DBT_PROJECT
}

/// The properties files' schema, parsed once.
pub fn dbt_yml_files() -> &'static Value {
    &DBT_YML_FILES
}
