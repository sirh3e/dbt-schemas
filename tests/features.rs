//! `Cargo.toml`'s `[features]` is what the upstream submodules hold: one
//! feature per YAML variant and per artifact version, groups of them, and the
//! default set. A new upstream schema makes this test fail until the table is
//! written again: `just features` (`DBT_SCHEMAS_WRITE=1 cargo test --test features`).

#[path = "../build/catalog.rs"]
#[allow(dead_code)]
mod catalog;

use std::collections::BTreeSet;
use std::path::Path;

use catalog::Kind;

/// The artifact families the default features build the newest version of,
/// beside the latest YAML schemas and the newest manifest projection.
const DEFAULT_FAMILIES: &[&str] = &["run-results", "sources", "catalog"];

fn list(features: &[String]) -> String {
    let quoted: Vec<String> = features.iter().map(|f| format!("\"{f}\"")).collect();
    let one_line = format!("[{}]", quoted.join(", "));
    if one_line.len() <= 80 {
        return one_line;
    }
    format!("[\n    {},\n]", quoted.join(",\n    "))
}

fn expected() -> String {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = catalog::discover(crate_dir);
    let features: BTreeSet<(Kind, String)> = entries
        .iter()
        .map(|e| (e.kind, e.feature.clone()))
        .collect();
    let of = |kinds: &[Kind]| -> Vec<String> {
        let mut out: Vec<String> = features
            .iter()
            .filter(|(k, _)| kinds.contains(k))
            .map(|(_, f)| f.clone())
            .collect();
        out.sort_by_key(|f| sort_key(f));
        out.dedup();
        out
    };
    let mut default = vec!["yaml-latest".to_string()];
    for family in DEFAULT_FAMILIES {
        if let Some(v) = catalog::newest(&entries, family) {
            default.push(format!("{family}-v{v}"));
        }
    }
    if let Some(v) = catalog::newest(&entries, "manifest") {
        default.push(format!("manifest-projection-v{v}"));
    }
    let manifests: Vec<String> = of(&[Kind::Artifact])
        .into_iter()
        .filter(|f| f.starts_with("manifest-v"))
        .collect();
    let artifacts: Vec<String> = of(&[Kind::Artifact, Kind::ManifestProjection])
        .into_iter()
        .filter(|f| !f.starts_with("manifest-v"))
        .collect();

    let mut out = String::from(
        "[features]\n\
         # Generated from the upstream submodules by tests/features.rs; `just features`\n\
         # writes it again. One feature per YAML variant and per artifact version.\n",
    );
    out.push_str(&format!("default = {}\n", list(&default)));
    out.push_str("# everything: every variant and version, complete manifests included\n");
    out.push_str(&format!(
        "all = {}\n",
        list(&["yaml-all", "artifacts-all", "manifests-full"].map(String::from))
    ));
    out.push_str(&format!("yaml-all = {}\n", list(&of(&[Kind::Yaml]))));
    out.push_str("# every artifact version, manifests as projections\n");
    out.push_str(&format!("artifacts-all = {}\n", list(&artifacts)));
    out.push_str(
        "# every manifest version's complete types (large: v12 alone is 186 KB of code)\n",
    );
    out.push_str(&format!("manifests-full = {}\n", list(&manifests)));
    let mut each = of(&[Kind::Yaml, Kind::Artifact, Kind::ManifestProjection]);
    each.sort_by_key(|f| sort_key(f));
    for feature in each {
        out.push_str(&format!("{feature} = []\n"));
    }
    out
}

/// `run-results-v10` after `run-results-v9`.
fn sort_key(feature: &str) -> (String, u32, String) {
    match feature
        .rsplit_once("-v")
        .and_then(|(head, v)| Some((head, v.parse::<u32>().ok()?)))
    {
        Some((head, v)) => (head.to_string(), v, String::new()),
        None => (feature.to_string(), 0, feature.to_string()),
    }
}

/// `Cargo.toml` with its `[features]` table replaced (or added before
/// `[dependencies]`).
fn with_features(manifest: &str, table: &str) -> String {
    let start = manifest.find("\n[features]\n").map(|i| i + 1);
    match start {
        Some(start) => {
            let rest = &manifest[start + 1..];
            let end = rest
                .find("\n[")
                .map(|i| start + 1 + i + 1)
                .unwrap_or(manifest.len());
            format!("{}{}\n{}", &manifest[..start], table, &manifest[end..])
        }
        None => {
            let at = manifest
                .find("\n[dependencies]\n")
                .map(|i| i + 1)
                .expect("Cargo.toml has [dependencies]");
            format!("{}{}\n{}", &manifest[..at], table, &manifest[at..])
        }
    }
}

#[test]
fn the_features_are_what_the_submodules_hold() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = std::fs::read_to_string(&path).unwrap();
    let wanted = with_features(&manifest, &expected());
    if std::env::var_os("DBT_SCHEMAS_WRITE").is_some() {
        std::fs::write(&path, &wanted).unwrap();
        return;
    }
    assert!(
        manifest == wanted,
        "Cargo.toml's [features] differs from what the submodules hold; run `just features`"
    );
}
