//! The rules of `build/avro.rs`, a JSON Schema in and an Avro schema out,
//! one case each. They need no feature: `build.rs` runs them only with
//! `avro`, but they are plain functions.

#[path = "../build/avro.rs"]
#[allow(dead_code)]
mod avro;

use avro::{avro_name, avro_schema};
use pretty_assertions::assert_eq;
use rstest::rstest;
use serde_json::{Value, json};

const NS: &str = "dbt_schemas.test.v1";

/// The type of the field `x` of a root record holding `schema` as `x`.
fn field_type(schema: Value, required: bool) -> Value {
    let root = json!({
        "title": "Root",
        "type": "object",
        "properties": {"x": schema},
        "required": if required { json!(["x"]) } else { json!([]) },
    });
    avro_schema(&root, NS)["fields"][0]["type"].clone()
}

fn any_json() -> Value {
    json!({"type": "string", "dbt_schemas.json": true})
}

#[rstest]
#[case::null(json!({"type": "null"}), json!("null"))]
#[case::boolean(json!({"type": "boolean"}), json!("boolean"))]
#[case::integer(json!({"type": "integer", "minimum": 0}), json!("long"))]
#[case::number(json!({"type": "number"}), json!("double"))]
#[case::string(json!({"type": "string", "format": "date-time", "pattern": "^x"}), json!("string"))]
#[case::enum_is_a_string(json!({"enum": ["pass", "warn", "error"]}), json!("string"))]
#[case::const_is_its_type(json!({"const": 2}), json!("long"))]
#[case::nothing_said_is_any_json(json!({}), any_json())]
#[case::true_is_any_json(json!(true), any_json())]
#[case::all_of_one_is_it(json!({"allOf": [{"type": "string"}]}), json!("string"))]
#[case::all_of_several_is_any_json(json!({"allOf": [{"type": "string"}, {"minLength": 1}]}), any_json())]
#[case::type_list_is_a_union(json!({"type": ["string", "null"]}), json!(["null", "string"]))]
#[case::any_of_is_a_union(
    json!({"anyOf": [{"type": "integer"}, {"type": "number"}, {"type": "null"}]}),
    json!(["null", "long", "double"])
)]
#[case::a_union_of_one_is_it(json!({"oneOf": [{"type": "string"}, {"type": "string"}]}), json!("string"))]
#[case::two_arrays_are_any_json(
    json!({"oneOf": [{"type": "array", "items": {"type": "string"}}, {"type": "array", "items": {"type": "integer"}}]}),
    any_json()
)]
#[case::any_json_beside_a_string_is_any_json(json!({"oneOf": [{"type": "string"}, {}]}), any_json())]
#[case::map_of_values(
    json!({"type": "object", "additionalProperties": {"type": "string"}}),
    json!({"type": "map", "values": "string"})
)]
#[case::map_of_anything(json!({"type": "object"}), json!({"type": "map", "values": any_json()}))]
#[case::map_of_patterns(
    json!({"type": "object", "patternProperties": {"^a": {"type": "integer"}}}),
    json!({"type": "map", "values": "long"})
)]
#[case::array(json!({"type": "array", "items": {"type": "boolean"}}), json!({"type": "array", "items": "boolean"}))]
#[case::tuple_as_one_union(
    json!({"type": "array", "prefixItems": [{"type": "string"}, {"type": "integer"}]}),
    json!({"type": "array", "items": ["string", "long"]})
)]
fn a_keyword_maps_to_one_avro_type(#[case] schema: Value, #[case] expected: Value) {
    assert_eq!(field_type(schema, true), expected);
}

#[rstest]
#[case::plain(json!({"type": "string"}), json!(["null", "string"]))]
#[case::null_moves_first(json!({"type": ["integer", "null"]}), json!(["null", "long"]))]
#[case::null_alone(json!({"type": "null"}), json!("null"))]
fn a_field_not_required_is_nullable(#[case] schema: Value, #[case] expected: Value) {
    assert_eq!(field_type(schema, false), expected);
}

#[test]
fn a_record_names_its_fields_and_keeps_the_names_avro_refuses() {
    let root = json!({
        "title": "Project",
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "model-paths": {"type": "array", "items": {"type": "string"}},
            "model_paths": {"type": "string"}
        },
        "required": ["name"]
    });
    assert_eq!(
        avro_schema(&root, NS),
        json!({
            "type": "record",
            "name": "Project",
            "namespace": NS,
            "fields": [
                {"name": "name", "type": "string"},
                {"name": "model_paths", "type": ["null", {"type": "array", "items": "string"}],
                 "default": null, "dbt_schemas.name": "model-paths"},
                {"name": "model_paths_2", "type": ["null", "string"], "default": null,
                 "dbt_schemas.name": "model_paths"}
            ]
        })
    );
}

#[test]
fn an_identical_record_is_defined_once_and_another_of_its_name_numbered() {
    let timing =
        json!({"title": "Timing", "type": "object", "properties": {"name": {"type": "string"}}});
    let other =
        json!({"title": "Timing", "type": "object", "properties": {"at": {"type": "string"}}});
    let root = json!({
        "title": "Root", "type": "object",
        "properties": {"a": timing, "b": timing, "c": other},
        "required": ["a", "b", "c"]
    });
    let fields = avro_schema(&root, NS)["fields"].clone();
    assert_eq!(fields[0]["type"]["name"], "Timing");
    assert_eq!(fields[1]["type"], json!(format!("{NS}.Timing")));
    assert_eq!(fields[2]["type"]["name"], "Timing_2");
}

#[test]
fn a_record_without_a_title_is_named_where_it_is() {
    let root = json!({
        "title": "Root", "type": "object",
        "properties": {"batch_results": {"type": "object", "properties": {"ok": {"type": "boolean"}}}},
        "required": ["batch_results"]
    });
    assert_eq!(
        avro_schema(&root, NS)["fields"][0]["type"]["name"],
        "BatchResults"
    );
}

#[test]
fn a_ref_to_a_record_is_defined_once_and_may_refer_to_itself() {
    let root = json!({
        "title": "Root", "type": "object",
        "properties": {
            "first": {"$ref": "#/definitions/Node"},
            "second": {"$ref": "#/$defs/Node"}
        },
        "required": ["first", "second"],
        "definitions": {"Node": {"type": "object", "properties": {
            "name": {"type": "string"},
            "children": {"type": "array", "items": {"$ref": "#/definitions/Node"}}
        }}},
        "$defs": {"Node": {"title": "Node", "type": "object", "properties": {"id": {"type": "integer"}}}}
    });
    let fields = avro_schema(&root, NS)["fields"].clone();
    assert_eq!(fields[0]["type"]["name"], "Node");
    assert_eq!(
        fields[0]["type"]["fields"][1]["type"],
        json!(["null", {"type": "array", "items": format!("{NS}.Node")}]),
        "a record refers to itself by name"
    );
    assert_eq!(
        fields[1]["type"]["name"], "Node_2",
        "another ref of the same name is another record"
    );
}

#[test]
fn a_ref_to_what_is_no_record_is_inlined_and_any_json_where_it_loops() {
    let root = json!({
        "title": "Root", "type": "object",
        "properties": {"tags": {"$ref": "#/definitions/Tags"}},
        "required": ["tags"],
        "definitions": {"Tags": {"anyOf": [
            {"type": "string"},
            {"type": "array", "items": {"$ref": "#/definitions/Tags"}}
        ]}}
    });
    assert_eq!(
        avro_schema(&root, NS)["fields"][0]["type"],
        json!(["string", {"type": "array", "items": any_json()}])
    );
}

#[test]
fn a_record_defined_in_a_union_that_falls_back_is_not_kept() {
    // two arrays make the union any JSON; the record inside the first must
    // not count as defined, or `later` would name a type nothing defines
    let inner =
        json!({"title": "Inner", "type": "object", "properties": {"a": {"type": "string"}}});
    let root = json!({
        "title": "Root", "type": "object",
        "properties": {
            "either": {"oneOf": [{"type": "array", "items": inner}, {"type": "array", "items": {"type": "string"}}]},
            "later": inner
        },
        "required": ["either", "later"]
    });
    let fields = avro_schema(&root, NS)["fields"].clone();
    assert_eq!(fields[0]["type"], any_json());
    assert_eq!(fields[1]["type"]["name"], "Inner");
}

#[rstest]
#[case::kept("run_results", "run_results")]
#[case::punctuation("post-hook", "post_hook")]
#[case::leading_digit("1_6", "_1_6")]
#[case::plus("+materialized", "_materialized")]
fn names_are_ones_avro_takes(#[case] text: &str, #[case] expected: &str) {
    assert_eq!(avro_name(text), expected);
}
