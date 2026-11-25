//! `Cargo.toml`'s `[features]` is what the upstream submodules hold: one
//! feature per YAML variant and per artifact version, groups of them, and the
//! default set. A new upstream schema makes this test fail until the table is
//! written again: `just features` (`DBT_SCHEMAS_WRITE=1 cargo test --test features`).

#[path = "../build/catalog.rs"]
#[allow(dead_code)]
mod catalog;
#[path = "../build/dbt_releases.rs"]
#[allow(dead_code)]
mod dbt_releases;

use std::collections::BTreeSet;
use std::path::Path;

use catalog::Kind;

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
    // the default features are what the newest dbt release reads
    // (`dbt-latest`): a schema version upstream adds moves them only once its
    // release is in build/dbt_releases.rs
    let default = dbt_releases::schema_features(latest_release());
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
    out.push_str(&dbt_features());
    let mut each = of(&[Kind::Yaml, Kind::Artifact, Kind::ManifestProjection]);
    each.sort_by_key(|f| sort_key(f));
    for feature in each {
        out.push_str(&format!("{feature} = []\n"));
    }
    out
}

/// The release `dbt-latest` names.
fn latest_release() -> &'static dbt_releases::DbtRelease {
    let (_, latest) = dbt_releases::ALIASES
        .iter()
        .find(|(alias, _)| *alias == "latest")
        .expect("build/dbt_releases.rs names a latest release");
    dbt_releases::RELEASES
        .iter()
        .find(|r| r.dbt == *latest)
        .expect("the latest release is a row")
}

/// The features by dbt release (`build/dbt_releases.rs`): one per release,
/// its aliases, and a group per major version.
fn dbt_features() -> String {
    let releases = dbt_releases::RELEASES;
    let mut out = String::from(
        "# by dbt release: what each writes and reads (build/dbt_releases.rs), manifests as\n\
         # projections\n",
    );
    let names: Vec<String> = releases
        .iter()
        .map(|r| dbt_releases::feature_of(r.dbt))
        .collect();
    out.push_str(&format!("dbt-all = {}\n", list(&names)));
    let mut majors: Vec<&str> = releases
        .iter()
        .map(|r| r.dbt.split('.').next().unwrap())
        .collect();
    majors.dedup();
    for major in majors {
        let members: Vec<String> = releases
            .iter()
            .filter(|r| r.dbt.split('.').next() == Some(major))
            .map(|r| dbt_releases::feature_of(r.dbt))
            .collect();
        out.push_str(&format!("dbt-{major} = {}\n", list(&members)));
    }
    for (alias, dbt) in dbt_releases::ALIASES {
        out.push_str(&format!(
            "dbt-{alias} = {}\n",
            list(&[dbt_releases::feature_of(dbt)])
        ));
    }
    for release in releases {
        out.push_str(&format!(
            "{} = {}\n",
            dbt_releases::feature_of(release.dbt),
            list(&dbt_releases::schema_features(release))
        ));
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

#[test]
fn the_default_features_are_the_latest_dbt_release() {
    let (_, latest) = dbt_releases::ALIASES
        .iter()
        .find(|(alias, _)| *alias == "latest")
        .unwrap();
    let release = dbt_releases::RELEASES
        .iter()
        .find(|r| r.dbt == *latest)
        .unwrap();
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    let default_line = manifest
        .split("\ndefault = ")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .unwrap();
    let mut defaults: Vec<&str> = default_line.split('"').skip(1).step_by(2).collect();
    defaults.sort();
    let mut wanted = dbt_releases::schema_features(release);
    wanted.sort();
    assert_eq!(
        defaults, wanted,
        "the default features are what dbt {latest} reads"
    );
}
