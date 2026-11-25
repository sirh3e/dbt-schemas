//! Every schema the upstream submodules hold, as the modules and features the
//! crate builds from them. Pure but for listing directories, so `build.rs`
//! and `tests/features.rs` read the same catalog.
//!
//! - dbt-jsonschema: `schemas/<variant>/<file>-<variant>.json`, one variant
//!   per directory (`1.5`, `1.6`, `1.7`, `latest`, `latest_fusion`; the
//!   Fusion files say `latest-fusion`). The loose files directly in
//!   `schemas/` are older copies of `latest` and are left out.
//! - schemas.getdbt.com: `dbt/<family>/v<N>.json`, every family and version.
//!   A manifest version gives two modules: its complete types and its
//!   projection (see `preprocess::manifest_projection`).
//!
//! A new upstream variant, family or version needs no code here: it becomes
//! a module and a feature by itself, and `tests/features.rs` asks for the
//! feature to be added to `Cargo.toml` (`just features`).

use std::path::{Path, PathBuf};

/// Where each upstream repository is checked out, below the crate.
pub const YAML_ROOT: &str = "schemas/upstream/dbt-jsonschema/schemas";
pub const ARTIFACT_ROOT: &str = "schemas/upstream/schemas.getdbt.com/dbt";

/// What a module is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A YAML file schema.
    Yaml,
    /// An artifact schema, complete (a manifest's too: `manifest-v<N>`).
    Artifact,
    /// A manifest's projection (`manifest-projection-v<N>`).
    ManifestProjection,
}

/// One module of `generated`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
    pub kind: Kind,
    /// `yaml/latest_fusion` or `manifest`: the module path above `name`.
    pub group: String,
    /// `dbt_project`, `v12`: the module's own name.
    pub name: String,
    /// The Cargo feature that builds it (`yaml-latest-fusion`, `run-results-v6`).
    pub feature: String,
    /// The pristine file, relative to the crate.
    pub path: PathBuf,
    /// The version of an artifact, for dispatch; none for a YAML file.
    pub version: Option<u32>,
    /// The artifact family as upstream names it (`run-results`); the YAML
    /// variant as its directory is named (`1.5`).
    pub family: String,
}

impl Entry {
    /// The file stem `build.rs` writes to `OUT_DIR`: unique across modules.
    pub fn stem(&self) -> String {
        format!("{}__{}", self.group.replace('/', "__"), self.name)
    }
}

/// A Rust module name of a variant or family: `1.5` → `v1_5`,
/// `run-results` → `run_results`.
pub fn module_of(name: &str) -> String {
    let ident = name.replace(['.', '-'], "_");
    if ident.starts_with(|c: char| c.is_ascii_digit()) {
        format!("v{ident}")
    } else {
        ident
    }
}

/// A feature name: `1.5` → `1-5`, `latest_fusion` → `latest-fusion`.
fn feature_of(name: &str) -> String {
    name.replace(['.', '_'], "-")
}

fn sorted_dir(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(|e| e.ok().map(|e| e.path())).collect(),
        Err(e) => panic!(
            "cannot list {}: {e} — are the submodules checked out? \
             (git submodule update --init)",
            dir.display()
        ),
    };
    paths.sort();
    paths
}

/// Every module the submodules allow, in a stable order.
pub fn discover(crate_dir: &Path) -> Vec<Entry> {
    let mut out = Vec::new();
    for dir in sorted_dir(&crate_dir.join(YAML_ROOT)) {
        if !dir.is_dir() {
            continue;
        }
        let variant = dir.file_name().unwrap().to_string_lossy().to_string();
        let suffix = format!("-{}.json", variant.replace('_', "-"));
        for file in sorted_dir(&dir) {
            let file_name = file.file_name().unwrap().to_string_lossy().to_string();
            let Some(name) = file_name.strip_suffix(&suffix) else {
                continue;
            };
            out.push(Entry {
                kind: Kind::Yaml,
                group: format!("yaml/{}", module_of(&variant)),
                name: name.to_string(),
                feature: format!("yaml-{}", feature_of(&variant)),
                path: file.strip_prefix(crate_dir).unwrap().to_path_buf(),
                version: None,
                family: variant.clone(),
            });
        }
    }
    for dir in sorted_dir(&crate_dir.join(ARTIFACT_ROOT)) {
        if !dir.is_dir() {
            continue;
        }
        let family = dir.file_name().unwrap().to_string_lossy().to_string();
        for file in sorted_dir(&dir) {
            let file_name = file.file_name().unwrap().to_string_lossy().to_string();
            let Some(version) = file_name
                .strip_prefix('v')
                .and_then(|v| v.strip_suffix(".json"))
                .and_then(|v| v.parse::<u32>().ok())
            else {
                continue;
            };
            let path = file.strip_prefix(crate_dir).unwrap().to_path_buf();
            let entry = |kind, group: String, feature: String| Entry {
                kind,
                group,
                name: format!("v{version}"),
                feature,
                path: path.clone(),
                version: Some(version),
                family: family.clone(),
            };
            out.push(entry(
                Kind::Artifact,
                module_of(&family),
                format!("{family}-v{version}"),
            ));
            if family == "manifest" {
                out.push(entry(
                    Kind::ManifestProjection,
                    "manifest/projection".into(),
                    format!("manifest-projection-v{version}"),
                ));
            }
        }
    }
    out.sort_by(|a, b| {
        (&a.group, a.version, &a.name, a.kind).cmp(&(&b.group, b.version, &b.name, b.kind))
    });
    out
}

/// The newest version of an artifact family, which the drift guards hold
/// to every name they list.
pub fn newest(entries: &[Entry], family: &str) -> Option<u32> {
    entries
        .iter()
        .filter(|e| e.family == family && e.version.is_some())
        .filter_map(|e| e.version)
        .max()
}
