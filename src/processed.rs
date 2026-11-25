//! The processed schemas themselves, as text and parsed, for what reads a
//! schema rather than a file: completion, highlighting, config keys.

#[cfg(feature = "yaml-latest")]
use std::sync::LazyLock;

use serde_json::Value;

pub use crate::generated::PROCESSED as ALL;

/// The processed schema of a module path (`yaml/v1_6/dbt_project`,
/// `manifest/projection/v12`), if this build has it.
pub fn schema(path: &str) -> Option<&'static str> {
    ALL.iter().find(|(p, _)| *p == path).map(|(_, text)| *text)
}

macro_rules! latest {
    ($name:ident, $file:literal, $doc:literal) => {
        #[doc = $doc]
        #[cfg(feature = "yaml-latest")]
        pub const $name: &str =
            include_str!(concat!(env!("OUT_DIR"), "/yaml__latest__", $file, ".json"));
    };
}

latest!(
    DBT_PROJECT_JSON,
    "dbt_project",
    "The processed latest `dbt_project.yml` schema."
);
latest!(
    DBT_YML_FILES_JSON,
    "dbt_yml_files",
    "The processed latest properties files' schema."
);
latest!(
    PACKAGES_JSON,
    "packages",
    "The processed latest `packages.yml` schema."
);
latest!(
    SELECTORS_JSON,
    "selectors",
    "The processed latest `selectors.yml` schema."
);

#[cfg(feature = "yaml-latest")]
static DBT_PROJECT: LazyLock<Value> = LazyLock::new(|| parse(DBT_PROJECT_JSON));
#[cfg(feature = "yaml-latest")]
static DBT_YML_FILES: LazyLock<Value> = LazyLock::new(|| parse(DBT_YML_FILES_JSON));

#[allow(dead_code)]
fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("a bundled schema is valid json")
}

/// The latest `dbt_project.yml` schema, parsed once.
#[cfg(feature = "yaml-latest")]
pub fn dbt_project() -> &'static Value {
    &DBT_PROJECT
}

/// The latest properties files' schema, parsed once.
#[cfg(feature = "yaml-latest")]
pub fn dbt_yml_files() -> &'static Value {
    &DBT_YML_FILES
}
