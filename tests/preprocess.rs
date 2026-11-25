//! The preprocessing rules of `build/preprocess.rs`, run here as `build.rs`
//! runs them, one case each.

#[path = "../build/preprocess.rs"]
#[allow(dead_code)]
mod preprocess;

use preprocess::{Mode, ident, manifest_projection, preprocess};
use pretty_assertions::assert_eq;
use rstest::rstest;
use serde_json::{Value, json};

fn run(mut schema: Value, mode: Mode) -> Value {
    preprocess(&mut schema, mode);
    schema
}

#[rstest]
#[case::plus_and_punctuation("+materialized", "materialized")]
#[case::dashes("model-paths", "model_paths")]
#[case::case_and_runs("A--B__c", "a_b_c")]
#[case::ends_trimmed("__x__", "x")]
#[case::nothing_left("+-", "")]
fn idents_sanitize_like_typify(#[case] key: &str, #[case] expected: &str) {
    assert_eq!(ident(key), expected);
}

#[rstest]
// rules 1 and 5: validation-only keywords and defaults go
#[case::if_then_else_default(
    json!({"type": "string", "if": {}, "then": {}, "else": {}, "default": "x"}),
    json!({"type": "string"})
)]
// rule 2: enum values differing only in case keep the first
#[case::enum_case_dedup(
    json!({"enum": ["table", "Table", "view", "TABLE"]}),
    json!({"enum": ["table", "view"]})
)]
#[case::mixed_enum_kept(json!({"enum": ["a", "A", 1]}), json!({"enum": ["a", "A", 1]}))]
// rule 3: the `+`-prefixed spelling of a key goes, wherever it stands
#[case::plus_after(
    json!({"properties": {"tags": {"type": "string"}, "+tags": {"type": "string"}}}),
    json!({"properties": {"tags": {"type": "string"}}})
)]
#[case::plus_before(
    json!({"properties": {"+tags": {"type": "string"}, "tags": {"type": "string"}}}),
    json!({"properties": {"tags": {"type": "string"}}})
)]
// rule 4: [X, null] collapses into the parent, its own keys first
#[case::null_branch_collapses(
    json!({"description": "d", "anyOf": [{"type": "string", "description": "inner"}, {"type": "null"}]}),
    json!({"description": "d", "type": "string"})
)]
#[case::const_null_collapses(
    json!({"oneOf": [{"const": null}, {"type": "integer"}]}),
    json!({"type": "integer"})
)]
#[case::three_branches_stay(
    json!({"anyOf": [{"type": "string"}, {"type": "integer"}, {"type": "null"}]}),
    json!({"anyOf": [{"type": "string"}, {"type": "integer"}, {"type": "null"}]})
)]
#[case::nested_nodes_too(
    json!({"properties": {"a": {"default": 1, "type": "integer"}}}),
    json!({"properties": {"a": {"type": "integer"}}})
)]
fn yaml_rules(#[case] schema: Value, #[case] expected: Value) {
    assert_eq!(run(schema, Mode::Yaml), expected);
}

#[rstest]
// rule (a): only the literal false goes; typed maps keep theirs
#[case::no_deny_unknown(
    json!({"type": "object", "additionalProperties": false}),
    json!({"type": "object"})
)]
#[case::typed_map_kept(
    json!({"type": "object", "additionalProperties": {"type": "integer"}}),
    json!({"type": "object", "additionalProperties": {"type": "integer"}})
)]
// rule (h): string-valued open maps take any value
#[case::string_map_relaxed(
    json!({"type": "object", "additionalProperties": {"type": "string"}}),
    json!({"type": "object", "additionalProperties": true})
)]
// rule (b): nothing is required
#[case::required_goes(
    json!({"type": "object", "required": ["a"], "properties": {"a": {"type": "string"}}}),
    json!({"type": "object", "properties": {"a": {"type": "string"}}})
)]
// rule (c): a union of string enums, inline or one $ref away, is a string
#[case::string_enum_union(
    json!({"anyOf": [{"enum": ["a"]}, {"$ref": "#/$defs/E"}], "$defs": {"E": {"enum": ["b"]}}}),
    json!({"$defs": {"E": {"enum": ["b"]}}, "type": "string"})
)]
// rules (d), (e), (f)
#[case::format_property_names_const(
    json!({"type": "string", "format": "date-time", "propertyNames": {}, "const": "v6"}),
    json!({"type": "string"})
)]
#[case::const_without_type(json!({"const": "v6"}), json!({"type": "string"}))]
// rule (g): a union of objects is one open object; a stringish clash is a string
#[case::object_union_merges(
    json!({"anyOf": [
        {"type": "object", "properties": {"status": {"enum": ["error"]}, "a": {"type": "integer"}}},
        {"type": "object", "properties": {"status": {"enum": ["pass"]}, "a": {"type": "number"}, "b": {"type": "string"}}}
    ]}),
    json!({"type": "object", "properties": {
        "status": {"type": "string"}, "a": {"type": "integer"}, "b": {"type": "string"}
    }})
)]
// rule (i): [X, null] stays, as dbt writes literal nulls
#[case::null_branch_kept(
    json!({"anyOf": [{"type": "array"}, {"type": "null"}]}),
    json!({"anyOf": [{"type": "array"}, {"type": "null"}]})
)]
// rule (j): named fields take any value
#[case::relax_to_any(
    json!({"title": "RunResultOutput", "properties": {"execution_time": {"type": "number"}, "x": {"type": "number"}}}),
    json!({"title": "RunResultOutput", "properties": {"execution_time": {}, "x": {"type": "number"}}})
)]
fn artifact_rules(#[case] schema: Value, #[case] expected: Value) {
    assert_eq!(run(schema, Mode::Artifact), expected);
}

#[test]
#[should_panic(expected = "RunResultOutput.execution_time is gone upstream")]
fn a_relaxed_field_gone_upstream_fails_the_build() {
    run(
        json!({"title": "RunResultOutput", "properties": {}}),
        Mode::Artifact,
    );
}

#[test]
fn keys_keep_the_pristine_order() {
    let schema = run(
        json!({"z": 1, "default": 0, "a": 2, "anyOf": [{"type": "string", "m": 3}, {"type": "null"}]}),
        Mode::Yaml,
    );
    let keys: Vec<&String> = schema.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["z", "a", "type", "m"]);
}

#[test]
fn the_projection_asserts_its_names_upstream() {
    let pristine = include_str!("../schemas/upstream/schemas.getdbt.com/dbt/manifest/v12.json");
    let projection = manifest_projection(pristine, 12, true);
    let node = &projection["properties"]["nodes"]["additionalProperties"]["properties"];
    for any in ["relation_name", "config", "columns"] {
        assert_eq!(node[any], json!({}), "{any} is any-valued");
    }
    assert!(node["checksum"]["properties"]["checksum"].is_object());
}

#[test]
#[should_panic(expected = "no longer appears in the pristine manifest v13 schema")]
fn a_projected_name_gone_upstream_fails_the_build() {
    manifest_projection(r#"{"nodes": {}}"#, 13, true);
}

#[test]
fn an_older_projection_keeps_what_its_version_has() {
    let projection = manifest_projection(r#"{"nodes": {"name": 1}, "metadata": {}}"#, 0, false);
    let properties = projection["properties"].as_object().unwrap();
    assert_eq!(properties.keys().collect::<Vec<_>>(), ["metadata", "nodes"]);
    let node = &properties["nodes"]["additionalProperties"]["properties"];
    assert_eq!(
        node.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["name"]
    );
    assert_eq!(
        projection["$id"],
        "https://schemas.getdbt.com/dbt/manifest/v0.json#dbt-schemas-projection"
    );
}
