//! Preprocess the vendored dbt schemas for typify 0.8.
//!
//! Run by `build.rs` on the pristine copies under `schemas/`; the output goes
//! to `OUT_DIR` and never into the repository. The rules were written in
//! cogitator and Noosphere, which each carried a copy, and are kept here once.
//!
//! typify limitations worked around in EVERY mode:
//!   1. if/then/else is unsupported            -> stripped (validation-only).
//!   2. enum values differing only in case     -> panic; deduped (keep first).
//!   3. `+alias` and `alias` style properties sanitize to one Rust ident
//!      -> duplicate struct fields; the `+`-prefixed spelling is dropped
//!      (unknown keys are ignored at deserialization, so files using the
//!      dropped spelling still parse).
//!   4. oneOf/anyOf of [X, null] names the inner type after the outer def ->
//!      duplicate type definitions; the null branch is dropped and X merged
//!      into the parent node (optional fields are Option<T> anyway, so a YAML
//!      `null` still deserializes to None).
//!   5. schema `default` values -> typify 0.8 emits broken default-constructor
//!      code for several; dropped, consumers apply their own defaults.
//!
//! [`Mode::Artifact`] adds the LENIENCY rules for the official artifact
//! schemas (schemas.getdbt.com), which are strict in ways dbt itself never
//! honors across versions:
//!   a. additionalProperties: false            -> removed (only the literal
//!      `false`; typed maps like catalog `nodes` keep theirs). Kills every
//!      deny_unknown_fields so unknown fields from other dbt versions pass.
//!   b. `required` lists                       -> removed entirely. Every
//!      field becomes Option + serde(default); a consumer re-imposes the one
//!      real requirement (unique_id) by DROPPING the row, never the file.
//!   c. anyOf/oneOf whose branches ALL resolve to string enums (inline, or a
//!      one-level $ref into $defs/definitions) -> plain {"type": "string"}.
//!      run-results `status` is three enum sets; a future status string must
//!      stay a String, not a rejected union variant.
//!   d. `format`                               -> removed. typify maps
//!      date-time/uuid to chrono/uuid foreign types; timestamps stay
//!      Option<String> in the stable API.
//!   e. `propertyNames`                        -> removed (env-style maps;
//!      validation-only, shaky typify support).
//!   f. a string-typed node carrying `const`   -> plain {"type": "string"}.
//!      Version pins must trace, never reject.
//!   g. anyOf/oneOf whose branches are ALL objects -> ONE merged open object
//!      (union of the branches' properties; a key defined by more than one
//!      branch becomes {"type": "string"} when every definition is
//!      string/string-enum, else the first definition wins). With `required`
//!      stripped, typify's untagged unions are order-sensitive over open
//!      objects — serde would match the first branch for every row
//!      (sources.json results: RuntimeError | Output).
//!   h. additionalProperties: {"type": "string"} -> true. env-style maps
//!      claim string values, but an odd value in a map that is never read
//!      (env, adapter_response) must not kill the file.
//!   i. the yaml-mode [X, null] collapse (rule 4) is SKIPPED: typify renders
//!      kept null branches as Option<X>, and dbt writes literal nulls
//!      ("errors": null) that a collapsed non-Option field rejects.
//!   j. the fields named in [`RELAX_TO_ANY`]  -> {} (any JSON value). The
//!      consumer reads them out of a serde_json::Value, so one odd stat
//!      cannot reject the whole artifact.
//!
//! [`manifest_projection`] ignores the input's structure: it ASSERTS every
//! projected field name still occurs in the pristine 946 KB manifest schema
//! (an upstream rename fails the build loudly instead of drifting silently),
//! then emits the union of the fields its consumers read. typifying the whole
//! schema would cost thousands of types and minutes of macro expansion.
//! `relation_name`, `config` and `columns` are emitted as {} (any JSON value)
//! so an adapter-specific shape degrades one field rather than the manifest.
//! A field a consumer needs is added to the `PROJECTION_*` tables.
//!
//! Keys keep the order of the pristine file (serde_json's `preserve_order`),
//! as rule (g)'s "first definition wins" depends on it.

use serde_json::{Map, Value, json};

/// How a schema is preprocessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// A dbt-jsonschema YAML file schema.
    Yaml,
    /// A schemas.getdbt.com artifact schema: the leniency rules too.
    Artifact,
}

/// Rule (j): by object title. A listed field missing from its object fails
/// the build, the same drift guard the manifest projection has.
pub const RELAX_TO_ANY: &[(&str, &[&str])] = &[
    ("CatalogTable", &["stats"]),
    ("CatalogMetadata", &["generated_at"]),
    ("RunResultOutput", &["execution_time"]),
];

/// Preprocess a whole schema in place.
pub fn preprocess(schema: &mut Value, mode: Mode) {
    // rules that resolve `$ref`s read the document as it was before the walk
    let root = schema.clone();
    walk(schema, &root, mode == Mode::Artifact);
}

/// `+materialized` and `materialized` sanitize alike: lower case, runs of
/// anything else one `_`, none at either end.
pub fn ident(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut gap = false;
    for c in lower.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if gap && !out.is_empty() {
                out.push('_');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    out
}

fn walk(node: &mut Value, root: &Value, artifact: bool) {
    match node {
        Value::Object(map) => {
            if artifact {
                patch_artifact_node(map, root);
            }
            // artifact mode keeps [X, null] branches: typify renders them as
            // proper Option<X>, and dbt WRITES nulls ("errors": null) that a
            // bare #[serde(default)] Vec would reject. The collapse exists
            // for dbt-jsonschema's duplicate-type-name quirk, a YAML-side
            // problem only.
            patch_node(map, artifact);
            for value in map.values_mut() {
                walk(value, root, artifact);
            }
        }
        Value::Array(items) => {
            for value in items {
                walk(value, root, artifact);
            }
        }
        _ => {}
    }
}

fn is_null_branch(branch: &Value) -> bool {
    branch.as_object().is_some_and(|b| {
        b.get("type") == Some(&Value::from("null")) || b.get("const") == Some(&Value::Null)
    })
}

/// One-level `$ref` into the same document's `$defs`/`definitions`.
fn resolve_ref<'a>(node: &'a Value, root: &'a Value) -> &'a Value {
    let Some(target) = node.get("$ref").and_then(Value::as_str) else {
        return node;
    };
    let Some(rest) = target.strip_prefix("#/") else {
        return node;
    };
    let Some((section, name)) = rest.split_once('/') else {
        return node;
    };
    if !matches!(section, "$defs" | "definitions") || name.is_empty() || name.contains('/') {
        return node;
    }
    root.get(section)
        .and_then(|defs| defs.get(name))
        .unwrap_or(node)
}

fn is_string_list(values: &[Value]) -> bool {
    values.iter().all(Value::is_string)
}

fn is_string_enum(node: &Value, root: &Value) -> bool {
    resolve_ref(node, root)
        .get("enum")
        .and_then(Value::as_array)
        .is_some_and(|e| is_string_list(e))
}

fn is_object_branch(node: &Value, root: &Value) -> bool {
    let node = resolve_ref(node, root);
    node.is_object()
        && (node.get("type") == Some(&Value::from("object")) || node.get("properties").is_some())
}

fn stringish(schema: &Value, root: &Value) -> bool {
    let schema = resolve_ref(schema, root);
    schema.is_object()
        && (schema.get("type") == Some(&Value::from("string")) || is_string_enum(schema, root))
}

fn patch_node(node: &mut Map<String, Value>, keep_null_branches: bool) {
    for key in ["if", "then", "else", "default"] {
        node.shift_remove(key);
    }
    if let Some(Value::Array(values)) = node.get_mut("enum")
        && is_string_list(values)
    {
        let mut seen = std::collections::HashSet::new();
        values.retain(|v| seen.insert(v.as_str().unwrap_or_default().to_lowercase()));
    }
    if let Some(Value::Object(props)) = node.get_mut("properties") {
        let mut by_ident: std::collections::HashMap<String, String> = Default::default();
        let keys: Vec<String> = props.keys().cloned().collect();
        for key in keys {
            let id = ident(&key);
            match by_ident.get(&id).cloned() {
                Some(earlier) => {
                    let plus = key.starts_with('+');
                    let (drop, keep) = if plus { (key, earlier) } else { (earlier, key) };
                    props.shift_remove(&drop);
                    by_ident.insert(id, keep);
                }
                None => {
                    by_ident.insert(id, key);
                }
            }
        }
    }
    if keep_null_branches {
        return;
    }
    for key in ["oneOf", "anyOf"] {
        let Some(Value::Array(branches)) = node.get(key) else {
            continue;
        };
        if branches.len() != 2 {
            continue;
        }
        let non_null: Vec<&Value> = branches.iter().filter(|b| !is_null_branch(b)).collect();
        let [Value::Object(inner)] = non_null.as_slice() else {
            continue;
        };
        let inner = inner.clone();
        node.shift_remove(key);
        for (k, v) in inner {
            node.entry(k).or_insert(v);
        }
    }
}

fn patch_artifact_node(node: &mut Map<String, Value>, root: &Value) {
    // rule (a): only the literal `false` — typed maps keep theirs
    if node.get("additionalProperties") == Some(&Value::Bool(false)) {
        node.shift_remove("additionalProperties");
    }
    // rule (h): string-valued open maps relax to any-valued
    if node.get("additionalProperties") == Some(&json!({"type": "string"})) {
        node.insert("additionalProperties".into(), Value::Bool(true));
    }
    // rule (b)
    node.shift_remove("required");
    // rule (c): all-string-enum unions collapse to a plain string
    for key in ["anyOf", "oneOf"] {
        let collapse = node
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|b| b.len() >= 2 && b.iter().all(|b| is_string_enum(b, root)));
        if collapse {
            node.shift_remove(key);
            node.shift_remove("enum");
            node.insert("type".into(), Value::from("string"));
        }
    }
    // rule (g): all-object unions merge into one open object
    for key in ["anyOf", "oneOf"] {
        let Some(Value::Array(branches)) = node.get(key) else {
            continue;
        };
        if !(branches.len() >= 2 && branches.iter().all(|b| is_object_branch(b, root))) {
            continue;
        }
        let mut merged = Map::new();
        for branch in branches {
            let branch = resolve_ref(branch, root);
            let Some(props) = branch.get("properties").and_then(Value::as_object) else {
                continue;
            };
            for (name, schema) in props {
                match merged.get(name) {
                    None => {
                        merged.insert(name.clone(), schema.clone());
                    }
                    Some(first) if first != schema => {
                        if stringish(first, root) && stringish(schema, root) {
                            merged.insert(name.clone(), json!({"type": "string"}));
                        }
                        // else: first definition wins
                    }
                    Some(_) => {}
                }
            }
        }
        node.shift_remove(key);
        node.insert("type".into(), Value::from("object"));
        node.insert("properties".into(), Value::Object(merged));
    }
    // rules (d) + (e)
    node.shift_remove("format");
    node.shift_remove("propertyNames");
    // rule (f)
    if node.get("const").is_some_and(Value::is_string) {
        node.shift_remove("const");
        node.entry("type").or_insert_with(|| Value::from("string"));
    }
    // rule (j)
    let title = node
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(title) = title {
        for (object, fields) in RELAX_TO_ANY {
            if *object != title {
                continue;
            }
            let props = node.get_mut("properties").and_then(Value::as_object_mut);
            for field in *fields {
                let slot = props.as_ref().is_some_and(|p| p.contains_key(*field));
                assert!(
                    slot,
                    "{title}.{field} is gone upstream; update RELAX_TO_ANY"
                );
            }
            if let Some(props) = props {
                for field in *fields {
                    props.insert((*field).to_string(), json!({}));
                }
            }
        }
    }
}

// ---- manifest projection ---------------------------------------------------
// The union of the manifest fields the consumers read: node identity, code,
// checksum, dependencies and config; sources; macros; the dependency maps;
// metadata. Field names drive both the drift assertion and the schema.

fn string() -> Value {
    json!({"type": "string"})
}

fn strings() -> Value {
    json!({"type": "array", "items": {"type": "string"}})
}

fn projection_meta_fields() -> Vec<(&'static str, Value)> {
    vec![
        ("dbt_schema_version", string()),
        ("dbt_version", string()),
        ("generated_at", string()),
        ("project_name", string()),
        ("adapter_type", string()),
    ]
}

fn projection_node_fields() -> Vec<(&'static str, Value)> {
    vec![
        ("unique_id", string()),
        ("name", string()),
        ("package_name", string()),
        ("resource_type", string()),
        ("original_file_path", string()),
        ("patch_path", string()),
        ("attached_node", string()),
        ("raw_code", string()),
        ("compiled_code", string()),
        ("tags", strings()),
        (
            "checksum",
            json!({
                "title": "ProjectionChecksum",
                "type": "object",
                "properties": {"name": string(), "checksum": string()},
            }),
        ),
        (
            "depends_on",
            json!({
                "title": "ProjectionDependsOn",
                "type": "object",
                "properties": {"nodes": strings(), "macros": strings()},
            }),
        ),
        // any-valued: adapter-specific shapes (relation_name is null for
        // ephemerals, config holds partition_by and the like upstream never
        // declares, columns' entries vary by version); read them out of the
        // serde_json::Value.
        ("relation_name", json!({})),
        ("config", json!({})),
        ("columns", json!({})),
    ]
}

/// Config keys consumers read that the pristine schema names; asserted only.
const PROJECTION_CONFIG_KEYS: &[&str] = &[
    "materialized",
    "enabled",
    "unique_key",
    "incremental_strategy",
];

fn projection_source_fields() -> Vec<(&'static str, Value)> {
    vec![
        ("unique_id", string()),
        ("source_name", string()),
        ("name", string()),
        ("package_name", string()),
        ("relation_name", string()),
        ("tags", strings()),
    ]
}

fn projection_macro_fields() -> Vec<(&'static str, Value)> {
    vec![
        ("unique_id", string()),
        ("name", string()),
        ("package_name", string()),
        ("macro_sql", string()),
        (
            "depends_on",
            json!({
                "title": "ProjectionMacroDependsOn",
                "type": "object",
                "properties": {"macros": strings()},
            }),
        ),
    ]
}

fn object(fields: Vec<(&'static str, Value)>) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

fn node_map(title: &str, fields: Vec<(&'static str, Value)>) -> Value {
    json!({
        "type": "object",
        "additionalProperties": {
            "title": title,
            "type": "object",
            "properties": object(fields),
        },
    })
}

/// The manifest schema consumers compile, out of the pristine manifest
/// schema's text. Panics, failing the build, when a projected name no longer
/// appears upstream.
pub fn manifest_projection(pristine_text: &str) -> Value {
    let names = |fields: Vec<(&'static str, Value)>| -> Vec<&'static str> {
        fields.into_iter().map(|(k, _)| k).collect()
    };
    let asserted = [
        names(projection_meta_fields()),
        names(projection_node_fields()),
        PROJECTION_CONFIG_KEYS.to_vec(),
        names(projection_source_fields()),
        names(projection_macro_fields()),
        vec![
            "nodes",
            "sources",
            "macros",
            "metadata",
            "parent_map",
            "child_map",
        ],
    ]
    .concat();
    for name in asserted {
        assert!(
            pristine_text.contains(&format!("\"{name}\"")),
            "projected field {name:?} no longer appears in the pristine manifest schema — \
             upstream renamed it; update the projection"
        );
    }
    let id_map = json!({"type": "object", "additionalProperties": strings()});
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://schemas.getdbt.com/dbt/manifest/v12.json#dbt-schemas-projection",
        "title": "ManifestProjection",
        "type": "object",
        "properties": {
            "metadata": {
                "title": "ManifestProjectionMetadata",
                "type": "object",
                "properties": object(projection_meta_fields()),
            },
            "nodes": node_map("ManifestProjectionNode", projection_node_fields()),
            "sources": node_map("ManifestProjectionSource", projection_source_fields()),
            "macros": node_map("ManifestProjectionMacro", projection_macro_fields()),
            "parent_map": id_map.clone(),
            "child_map": id_map,
        },
    })
}
