//! dbt releases, and the schemas each writes and reads.
//!
//! The same table the `dbt-<major>-<minor>` features are made of
//! (`build/dbt_releases.rs`, which says where every row was read): turn on
//! `dbt-1-7` and the build has everything dbt 1.7 writes; ask
//! [`release`] which schemas a project's dbt version means.
//!
//! ```ignore
//! let release = dbt_schemas::dbt::release("1.7.14").unwrap();
//! assert_eq!(release.schema("manifest"), Some(11));
//! assert_eq!(release.feature, "dbt-1-7");
//! ```

pub use crate::generated::__dbt::{ALIASES, RELEASES};

/// One dbt release and the schemas it writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct DbtRelease {
    /// `major.minor`: `1.7`, `2.0`.
    pub dbt: &'static str,
    /// Its Cargo feature: `dbt-1-7`.
    pub feature: &'static str,
    /// Each artifact family it writes and the schema version of each.
    pub artifacts: &'static [(&'static str, u32)],
    /// dbt-jsonschema's directory of its YAML schemas (`1.7`, `latest`), if any.
    pub yaml: Option<&'static str>,
    /// The per-schema features its feature turns on.
    pub features: &'static [&'static str],
    /// Whether this build has every one of them.
    pub built: bool,
    /// Where the row was read: dbt-core's source at a tag, or dbt's docs.
    pub source: &'static str,
}

impl DbtRelease {
    /// The schema version of an artifact family it writes: `manifest` → 11.
    pub fn schema(&self, family: &str) -> Option<u32> {
        self.artifacts
            .iter()
            .find(|(f, _)| *f == family)
            .map(|(_, v)| *v)
    }
}

/// The release of a dbt version, by its `major.minor`: `1.7.14`, `1.7.0rc1`,
/// `v2.0.0-beta.1`; or an alias, `latest` or `fusion`. None for a release the
/// table does not know.
pub fn release(version: &str) -> Option<&'static DbtRelease> {
    let version = version.trim().trim_start_matches('v');
    if let Some((_, dbt)) = ALIASES.iter().find(|(alias, _)| *alias == version) {
        return RELEASES.iter().find(|r| r.dbt == *dbt);
    }
    let mut parts = version.split(|c: char| !c.is_ascii_digit());
    let major = parts.next().filter(|p| !p.is_empty())?;
    let minor = parts.next().filter(|p| !p.is_empty())?;
    let key = format!(
        "{}.{}",
        major.parse::<u32>().ok()?,
        minor.parse::<u32>().ok()?
    );
    RELEASES.iter().find(|r| r.dbt == key)
}

/// The releases that write a schema version: `("manifest", 12)` → 1.8 … 2.0.
pub fn releases_of(family: &str, version: u32) -> impl Iterator<Item = &'static DbtRelease> {
    let family = family.to_string();
    RELEASES
        .iter()
        .filter(move |r| r.schema(&family) == Some(version))
}

/// The releases a dbt artifact can be from: the one its
/// `metadata.dbt_version` names when the table knows it, else every release
/// that writes the schema its `metadata.dbt_schema_version` names.
pub fn releases_of_document(value: &serde_json::Value) -> Vec<&'static DbtRelease> {
    let named = value
        .get("metadata")
        .and_then(|m| m.get("dbt_version"))
        .and_then(serde_json::Value::as_str)
        .and_then(release);
    if let Some(release) = named {
        return vec![release];
    }
    match crate::artifacts::sniff(value) {
        Some(schema) => releases_of(&schema.family, schema.version).collect(),
        None => Vec::new(),
    }
}
