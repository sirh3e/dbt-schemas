//! JSON artifact types of every version this build has, and the dispatch that
//! reads a document as the version it names.
//!
//! Every field is optional and unknown fields pass, so one type reads the
//! files of many dbt versions. `run_results`, `sources`, `catalog` and
//! `freshness` re-export their newest version at their root and hold each
//! version as `v<N>`; `manifest` re-exports the newest projection and holds
//! `projection::v<N>` and, with the `manifest-v<N>` features, the complete
//! types as `v<N>`.
//!
//! ```ignore
//! use dbt_schemas::artifacts::AnyRunResults;
//!
//! match AnyRunResults::from_value(value)? {
//!     AnyRunResults::V6(results) => println!("{} results", results.results.len()),
//!     AnyRunResults::V5(results) => println!("{} results", results.results.len()),
//!     other => println!("run results v{}", other.version()),
//! }
//! ```

#[allow(unused_imports)] // a build of no artifact features has none
pub use crate::generated::__artifacts::*;
pub use crate::generated::SCHEMA_IDS;

/// The `$id` of each artifact schema the default features build.
pub const MANIFEST_SCHEMA_V12: &str = "https://schemas.getdbt.com/dbt/manifest/v12.json";
pub const RUN_RESULTS_SCHEMA_V6: &str = "https://schemas.getdbt.com/dbt/run-results/v6.json";
pub const SOURCES_SCHEMA_V3: &str = "https://schemas.getdbt.com/dbt/sources/v3.json";
pub const CATALOG_SCHEMA_V1: &str = "https://schemas.getdbt.com/dbt/catalog/v1.json";

/// Which schema an artifact says it was written in.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SchemaVersion {
    /// The family as upstream names it: `manifest`, `run-results`, …
    pub family: String,
    pub version: u32,
}

/// Why a document could not be read as a version.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DispatchError {
    /// It has no `metadata.dbt_schema_version` of schemas.getdbt.com.
    #[error("the document names no dbt artifact schema (metadata.dbt_schema_version)")]
    NoVersion,
    /// It is an artifact of another family.
    #[error("the document is a {found} artifact, not a {expected} one")]
    OtherFamily {
        expected: &'static str,
        found: String,
    },
    /// This build has no module of that version: its feature is off, or
    /// upstream had no such schema when this crate was released.
    #[error("{family} v{version} is not in this build (feature `{family}-v{version}`)")]
    NotBuilt { family: &'static str, version: u32 },
    /// It does not fit the version it names.
    #[error("the document does not fit its schema: {0}")]
    Shape(#[source] serde_json::Error),
}

/// The schema a document names in `metadata.dbt_schema_version`
/// (`https://schemas.getdbt.com/dbt/manifest/v12.json`), if any.
pub fn sniff(value: &serde_json::Value) -> Option<SchemaVersion> {
    let id = value.get("metadata")?.get("dbt_schema_version")?.as_str()?;
    let rest = id
        .strip_prefix("https://schemas.getdbt.com/dbt/")
        .or_else(|| id.strip_prefix("http://schemas.getdbt.com/dbt/"))?;
    let (family, file) = rest.split_once('/')?;
    let version = file
        .strip_prefix('v')?
        .strip_suffix(".json")?
        .parse()
        .ok()?;
    Some(SchemaVersion {
        family: family.to_string(),
        version,
    })
}

/// The dbt releases a document can be from (see [`crate::dbt`]).
pub fn dbt_releases(value: &serde_json::Value) -> Vec<&'static crate::dbt::DbtRelease> {
    crate::dbt::releases_of_document(value)
}

/// The version of a document that must be of `family`.
pub fn sniff_family(value: &serde_json::Value, family: &'static str) -> Result<u32, DispatchError> {
    let found = sniff(value).ok_or(DispatchError::NoVersion)?;
    if found.family != family {
        return Err(DispatchError::OtherFamily {
            expected: family,
            found: found.family,
        });
    }
    Ok(found.version)
}
