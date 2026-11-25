#![cfg(feature = "avro")]
//! The `avro` feature: every schema built is an Avro schema apache-avro
//! takes, and documents go through it and back.

use dbt_schemas::avro::{self, AvroError};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[test]
fn every_schema_built_is_an_avro_schema() {
    let refused: Vec<String> = avro::ALL
        .iter()
        .filter_map(|(path, text)| {
            avro::AvroSchema::parse(path, text)
                .err()
                .map(|e| format!("{path}: {e}"))
        })
        .collect();
    assert!(refused.is_empty(), "{refused:#?}");
}

/// What a document reads back as: the fields the schema knows, without nulls.
fn round_trip(path: &str, document: &Value) -> Value {
    let schema = avro::schema(path).unwrap();
    schema.decode(&schema.encode(document).unwrap()).unwrap()
}

#[cfg(feature = "run-results-v6")]
#[test]
fn run_results_go_through_avro_and_back() {
    let document = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/run-results/v6.json",
            "dbt_version": "1.11.0",
            "generated_at": "2026-11-25T10:00:00Z",
            "invocation_id": null,
            "env": {"CI": "1", "depth": 2}
        },
        "results": [{
            "unique_id": "model.shop.orders",
            "status": "success",
            "thread_id": "Thread-1",
            "execution_time": 1.5,
            "timing": [{"name": "compile", "started_at": "a", "completed_at": "b"}],
            "adapter_response": {"_message": "OK", "rows_affected": 3},
            "failures": 0,
            "batch_results": {"successful": [["2026-11-01", "2026-11-02"]], "failed": []},
            "future_field": {"x": 1}
        }],
        "elapsed_time": 3,
        "args": {"which": "build", "select": ["orders"]},
        "extra": true
    });
    let mut expected = document.clone();
    // Avro keeps the fields the schema names, and a null is no field
    expected["metadata"]
        .as_object_mut()
        .unwrap()
        .remove("invocation_id");
    expected["results"][0]
        .as_object_mut()
        .unwrap()
        .remove("future_field");
    expected.as_object_mut().unwrap().remove("extra");
    assert_eq!(round_trip("run_results/v6", &document), expected);
}

#[cfg(feature = "run-results-v6")]
#[test]
fn a_document_finds_its_schema_by_the_version_it_names() {
    let document = json!({
        "metadata": {"dbt_schema_version": "https://schemas.getdbt.com/dbt/run-results/v6.json"}
    });
    assert_eq!(avro::schema_of(&document).unwrap().path(), "run_results/v6");
    assert!(matches!(
        avro::schema_of(&json!({})),
        Err(AvroError::NoVersion)
    ));
    let unbuilt = json!({"metadata": {"dbt_schema_version": "https://schemas.getdbt.com/dbt/run-results/v99.json"}});
    assert!(matches!(
        avro::schema_of(&unbuilt),
        Err(AvroError::NotBuilt(_))
    ));
}

#[cfg(feature = "run-results-v6")]
#[test]
fn a_value_that_does_not_fit_says_where() {
    let schema = avro::schema("run_results/v6").unwrap();
    let error = schema
        .encode(&json!({"results": [{"failures": "many"}]}))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "at $.results[0].failures: a string does not fit [null, long]"
    );
}

#[cfg(feature = "manifest-projection-v12")]
#[test]
fn a_manifest_projection_goes_through_avro_and_back() {
    let document = json!({
        "metadata": {"dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json"},
        "nodes": {
            "model.shop.orders": {
                "unique_id": "model.shop.orders",
                "resource_type": "model",
                "name": "orders",
                "fqn": ["shop", "orders"],
                "tags": ["daily"],
                "config": {"materialized": "table", "tags": ["daily"], "meta": {"owner": null}},
                "columns": {"id": {"name": "id", "data_type": "bigint"}},
                "depends_on": {"nodes": ["source.shop.raw.orders"], "macros": []}
            }
        },
        "parent_map": {"model.shop.orders": ["source.shop.raw.orders"]},
        "child_map": {"source.shop.raw.orders": ["model.shop.orders"]}
    });
    let mut expected = document.clone();
    // the projection keeps no fqn
    expected["nodes"]["model.shop.orders"]
        .as_object_mut()
        .unwrap()
        .remove("fqn");
    assert_eq!(round_trip("manifest/projection/v12", &document), expected);
}

#[cfg(feature = "sources-v3")]
#[test]
fn sources_go_through_avro_and_back() {
    let document = json!({
        "metadata": {"dbt_schema_version": "https://schemas.getdbt.com/dbt/sources/v3.json"},
        "results": [
            {"unique_id": "source.p.s.ok", "status": "pass", "snapshotted_at": "t1", "max_loaded_at": "t0"},
            {"unique_id": "source.p.s.boom", "status": "runtime error", "error": "db gone"}
        ],
        "elapsed_time": 0.25
    });
    assert_eq!(round_trip("sources/v3", &document), document);
}

#[cfg(feature = "yaml-latest")]
#[test]
fn a_field_avro_refuses_keeps_the_name_it_stands_for() {
    let schema = avro::schema("yaml/latest/dbt_project").unwrap();
    let text = schema.text();
    assert!(
        text.contains(avro::NAME_ATTR),
        "some dbt_project key is no Avro name"
    );
    let document = json!({"name": "shop", "model-paths": ["models"], "config-version": 2});
    assert_eq!(
        schema.decode(&schema.encode(&document).unwrap()).unwrap(),
        document
    );
}

/// A document as the generated types write one: no nulls, and no empty
/// lists (they skip those; Avro keeps them).
#[cfg(feature = "manifest-v12")]
fn as_written(value: Value) -> Value {
    match value {
        Value::Object(o) => Value::Object(
            o.into_iter()
                .filter(|(_, v)| !v.is_null() && v.as_array().is_none_or(|a| !a.is_empty()))
                .map(|(k, v)| (k, as_written(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.into_iter().map(as_written).collect()),
        other => other,
    }
}

#[cfg(feature = "manifest-v12")]
#[test]
fn a_complete_manifest_reads_back_as_the_generated_types_read_it() {
    use dbt_schemas::artifacts::AnyManifest;
    let schema = avro::schema("manifest/v12").unwrap();
    let node = |id: &str, kind: &str| {
        json!({
            "unique_id": id, "resource_type": kind, "name": "n", "package_name": "shop",
            "path": "p", "original_file_path": "o", "fqn": ["shop", "n"], "alias": "n",
            "schema": "main", "database": "db", "checksum": {"name": "sha256", "checksum": "abc"},
            "language": "sql", "raw_code": "select 1", "config": {"materialized": "table"},
            "depends_on": {"nodes": ["seed.shop.countries"], "macros": []}
        })
    };
    let document = json!({
        "metadata": {"dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json"},
        "nodes": {
            "seed.shop.countries": node("seed.shop.countries", "seed"),
            "model.shop.orders": node("model.shop.orders", "model")
        },
        "parent_map": {"model.shop.orders": ["seed.shop.countries"]}
    });
    let AnyManifest::V12(typed) = AnyManifest::from_value(document.clone()).unwrap() else {
        panic!("a v12 manifest reads as v12");
    };
    // the node union reads merged, the first definition winning (rule (g)
    // of build/preprocess.rs): a model keeps a seed's depends_on, in both
    let kept = as_written(serde_json::to_value(&typed).unwrap());
    let back = as_written(schema.decode(&schema.encode(&document).unwrap()).unwrap());
    assert_eq!(back, kept);
}
