//! The processed YAML schemas themselves, as text and parsed, for what
//! reads a schema rather than a file: completion, highlighting, config keys.

use std::sync::LazyLock;

use serde_json::Value;

/// `schemas/yaml/processed/dbt_project.json`.
pub const DBT_PROJECT_JSON: &str = include_str!("../schemas/yaml/processed/dbt_project.json");
/// `schemas/yaml/processed/dbt_yml_files.json`, the properties files.
pub const DBT_YML_FILES_JSON: &str = include_str!("../schemas/yaml/processed/dbt_yml_files.json");
/// `schemas/yaml/processed/packages.json`.
pub const PACKAGES_JSON: &str = include_str!("../schemas/yaml/processed/packages.json");
/// `schemas/yaml/processed/selectors.json`.
pub const SELECTORS_JSON: &str = include_str!("../schemas/yaml/processed/selectors.json");

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
