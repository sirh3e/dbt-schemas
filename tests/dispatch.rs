//! Reading an artifact as the version it names: `sniff`, and the `Any<Family>`
//! enum of each family, which has a variant for every version this build has.

#[allow(unused_imports)] // with no artifact features only sniff is tested
use dbt_schemas::artifacts::{self, DispatchError, SchemaVersion};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn written_in(family: &str, version: u32) -> Value {
    json!({"metadata": {"dbt_schema_version": format!("https://schemas.getdbt.com/dbt/{family}/v{version}.json")}})
}

#[test]
fn sniff_reads_the_schema_a_document_names() {
    assert_eq!(
        artifacts::sniff(&written_in("run-results", 5)),
        Some(SchemaVersion {
            family: "run-results".into(),
            version: 5
        })
    );
    assert_eq!(artifacts::sniff(&json!({"metadata": {}})), None);
    assert_eq!(
        artifacts::sniff(
            &json!({"metadata": {"dbt_schema_version": "https://example.com/v1.json"}})
        ),
        None
    );
}

#[test]
fn every_upstream_schema_has_its_id() {
    assert!(artifacts::SCHEMA_IDS.contains(&"https://schemas.getdbt.com/dbt/manifest/v12.json"));
    assert!(artifacts::SCHEMA_IDS.contains(&"https://schemas.getdbt.com/dbt/run-results/v0.json"));
}

#[cfg(feature = "run-results-v6")]
#[test]
fn a_document_is_read_as_its_version() {
    use artifacts::AnyRunResults;
    let read = AnyRunResults::from_value(written_in("run-results", 6)).unwrap();
    assert_eq!(read.version(), 6);
    assert!(matches!(read, AnyRunResults::V6(_)));
}

#[cfg(feature = "run-results-v6")]
#[test]
fn a_dispatch_says_why_it_cannot_read() {
    use artifacts::AnyRunResults;
    let other = AnyRunResults::from_value(written_in("manifest", 12)).unwrap_err();
    assert!(
        matches!(other, DispatchError::OtherFamily { .. }),
        "{other}"
    );
    let none = AnyRunResults::from_value(json!({})).unwrap_err();
    assert!(matches!(none, DispatchError::NoVersion), "{none}");
    let future = AnyRunResults::from_value(written_in("run-results", 99)).unwrap_err();
    assert!(
        matches!(future, DispatchError::NotBuilt { version: 99, .. }),
        "{future}"
    );
}

/// With every feature, every version upstream has is read as itself.
#[cfg(feature = "artifacts-all")]
#[test]
fn every_version_of_every_family_dispatches() {
    use artifacts::{AnyCatalog, AnyFreshness, AnyManifestProjection, AnyRunResults, AnySources};
    for id in artifacts::SCHEMA_IDS {
        let file = id.trim_start_matches("https://schemas.getdbt.com/dbt/");
        let (family, rest) = file.split_once('/').unwrap();
        let version: u32 = rest
            .trim_start_matches('v')
            .trim_end_matches(".json")
            .parse()
            .unwrap();
        let doc = written_in(family, version);
        let read = match family {
            "run-results" => AnyRunResults::from_value(doc).map(|d| d.version()),
            "sources" => AnySources::from_value(doc).map(|d| d.version()),
            "catalog" => AnyCatalog::from_value(doc).map(|d| d.version()),
            "freshness" => AnyFreshness::from_value(doc).map(|d| d.version()),
            "manifest" => AnyManifestProjection::from_value(doc).map(|d| d.version()),
            other => panic!("a family this test does not know: {other}"),
        };
        match read {
            Ok(v) => assert_eq!(v, version, "{id}"),
            // an old schema whose root is strict about a field: fine, as long
            // as it was the right version that refused
            Err(DispatchError::Shape(_)) => {}
            Err(e) => panic!("{id}: {e}"),
        }
    }
}
