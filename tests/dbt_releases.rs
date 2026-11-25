//! The dbt releases: the table is right about upstream, and the lookups find
//! a release by any way of writing its version.

#[path = "../build/catalog.rs"]
#[allow(dead_code)]
mod catalog;
#[path = "../build/dbt_releases.rs"]
#[allow(dead_code)]
mod table;

use std::path::Path;

use dbt_schemas::dbt;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn every_schema_a_release_names_is_upstream() {
    let entries = catalog::discover(Path::new(env!("CARGO_MANIFEST_DIR")));
    let features: Vec<&str> = entries.iter().map(|e| e.feature.as_str()).collect();
    for release in table::RELEASES {
        for feature in table::schema_features(release) {
            assert!(
                features.contains(&feature.as_str()),
                "dbt {} names `{feature}`, which upstream does not have",
                release.dbt
            );
        }
    }
}

#[test]
fn releases_are_in_order_and_aliases_name_one() {
    let key = |dbt: &str| -> (u32, u32) {
        let (major, minor) = dbt.split_once('.').unwrap();
        (major.parse().unwrap(), minor.parse().unwrap())
    };
    let keys: Vec<(u32, u32)> = table::RELEASES.iter().map(|r| key(r.dbt)).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(keys, sorted, "releases are oldest first, each once");
    for (alias, dbt) in table::ALIASES {
        assert!(
            table::RELEASES.iter().any(|r| r.dbt == *dbt),
            "dbt-{alias} names dbt {dbt}, which is no release"
        );
    }
}

#[test]
fn the_runtime_table_is_the_build_table() {
    assert_eq!(dbt::RELEASES.len(), table::RELEASES.len());
    for (runtime, built) in dbt::RELEASES.iter().zip(table::RELEASES) {
        assert_eq!(runtime.dbt, built.dbt);
        assert_eq!(runtime.artifacts, built.artifacts);
        assert_eq!(runtime.features, table::schema_features(built));
    }
}

#[test]
fn a_release_is_found_however_its_version_is_written() {
    let release = dbt::release("1.7.14").unwrap();
    assert_eq!(release.dbt, "1.7");
    assert_eq!(release.feature, "dbt-1-7");
    assert_eq!(release.schema("manifest"), Some(11));
    assert_eq!(release.schema("run-results"), Some(5));
    assert_eq!(release.yaml, Some("1.7"));
    for written in ["1.7", "1.7.0rc1", "v1.7.2", " 1.7.0b2 "] {
        assert_eq!(
            dbt::release(written).map(|r| r.dbt),
            Some("1.7"),
            "{written}"
        );
    }
    assert_eq!(dbt::release("2.0.0-beta.1").map(|r| r.dbt), Some("2.0"));
    assert_eq!(dbt::release("1.10.3").map(|r| r.dbt), Some("1.10"));
    assert_eq!(dbt::release("latest").map(|r| r.dbt), Some("1.11"));
    assert_eq!(dbt::release("fusion").map(|r| r.dbt), Some("2.0"));
    for unknown in ["0.18.1", "1.99.0", "", "dbt", "1"] {
        assert_eq!(dbt::release(unknown), None, "{unknown}");
    }
}

#[test]
fn a_schema_version_names_the_releases_that_write_it() {
    let releases: Vec<&str> = dbt::releases_of("manifest", 12).map(|r| r.dbt).collect();
    assert_eq!(releases, ["1.8", "1.9", "1.10", "1.11", "2.0"]);
    let run_results: Vec<&str> = dbt::releases_of("run-results", 4).map(|r| r.dbt).collect();
    assert_eq!(
        run_results,
        ["1.0", "1.1", "1.2", "1.3", "1.4", "1.5", "1.6"]
    );
    assert_eq!(
        dbt::releases_of("manifest", 0).count(),
        0,
        "v0 belongs to no release"
    );
    let freshness: Vec<&str> = dbt::releases_of("freshness", 0).map(|r| r.dbt).collect();
    assert_eq!(freshness, ["2.0"]);
}

#[test]
fn a_document_says_which_release_wrote_it() {
    let named = json!({"metadata": {
        "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
        "dbt_version": "1.9.4"
    }});
    let releases: Vec<&str> = dbt_schemas::artifacts::dbt_releases(&named)
        .iter()
        .map(|r| r.dbt)
        .collect();
    assert_eq!(releases, ["1.9"]);
    let schema_only = json!({"metadata": {
        "dbt_schema_version": "https://schemas.getdbt.com/dbt/run-results/v5.json"
    }});
    let releases: Vec<&str> = dbt_schemas::artifacts::dbt_releases(&schema_only)
        .iter()
        .map(|r| r.dbt)
        .collect();
    assert_eq!(releases, ["1.7"]);
    assert!(dbt_schemas::artifacts::dbt_releases(&json!({})).is_empty());
}

#[test]
fn a_release_is_built_when_its_features_are() {
    let latest = dbt::release("latest").unwrap();
    // the default features are dbt-latest's (tests/features.rs holds that)
    assert_eq!(
        latest.built,
        cfg!(feature = "dbt-1-11")
            || cfg!(all(
                feature = "yaml-latest",
                feature = "run-results-v6",
                feature = "sources-v3",
                feature = "catalog-v1",
                feature = "manifest-projection-v12"
            ))
    );
    assert_eq!(
        dbt::release("1.7").unwrap().built,
        cfg!(all(
            feature = "manifest-projection-v11",
            feature = "run-results-v5",
            feature = "sources-v3",
            feature = "catalog-v1",
            feature = "yaml-1-7"
        ))
    );
}
