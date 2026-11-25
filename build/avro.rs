//! A processed JSON Schema as an Avro schema (`.avsc`), for the `avro` feature.
//!
//! The Avro schema is made from the schema `build/preprocess.rs` wrote, so it
//! is exactly as lenient: a field the schema does not require is
//! `["null", T]` with a `null` default. JSON Schema says more than Avro can,
//! so each keyword has one rule:
//!
//! | JSON Schema | Avro |
//! |---|---|
//! | `null`, `boolean`, `integer`, `number`, `string` | `null`, `boolean`, `long`, `double`, `string` |
//! | `enum`, `const`, `format`, `pattern` | the type of their values: an enum is a `string`, as the artifact rules read statuses as strings |
//! | an object with `properties` | a `record`, named by its `title` or where it is |
//! | an object without `properties` | a `map` of `additionalProperties` (or `patternProperties`) |
//! | `items`, `prefixItems` | an `array` of the items (a tuple's items as one union) |
//! | `oneOf`, `anyOf`, a list of types | a `union`, `null` first |
//! | `$ref` | the named record it is, defined where it is first met; a recursive `$ref` to what is no record is any JSON |
//! | `{}`, `true`, `allOf` of several, `not` | any JSON |
//!
//! **Any JSON** is a `string` holding the value's JSON text, marked
//! `"dbt_schemas.json": true` (Avro readers pass unknown attributes by). A
//! union Avro cannot hold (two arrays, two maps, or any JSON beside another
//! string) is any JSON as a whole. A field name Avro refuses (`post-hook`)
//! is written as one it takes (`post_hook`) with the name it stands for in
//! `"dbt_schemas.name"`. Record names are unique within a schema: an inline
//! object identical to a record already defined is that record, another one
//! of the same name gets a `_2`.
//!
//! Everything here is pure: a schema in, an Avro schema out, the names
//! defined so far threaded through as a value.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

/// The attribute that marks a `string` holding any JSON value's text.
pub const JSON_ATTR: &str = "dbt_schemas.json";
/// The attribute that keeps a field's name where Avro refused it.
pub const NAME_ATTR: &str = "dbt_schemas.name";

/// The Avro schema of a processed JSON Schema, every named type in `namespace`.
pub fn avro_schema(root: &Value, namespace: &str) -> Value {
    let doc = Doc { root, namespace };
    convert(&doc, root, "Root", &Names::default()).0
}

/// The schema being converted: what a `$ref` resolves against.
struct Doc<'a> {
    root: &'a Value,
    namespace: &'a str,
}

/// The named records defined so far, and the `$ref`s being converted.
#[derive(Clone, Default)]
struct Names {
    /// A record's name to its fields (`None` while its `$ref` is being
    /// converted), so that an identical inline record is the same one.
    defined: BTreeMap<String, Option<Value>>,
    /// A `$ref` that is a record, to that record's name.
    refs: BTreeMap<String, String>,
    /// The `$ref`s being converted, outermost first.
    open: Vec<String>,
}

type Converted = (Value, Names);

fn convert(doc: &Doc, node: &Value, hint: &str, names: &Names) -> Converted {
    match node {
        Value::Object(map) => convert_object(doc, map, hint, names),
        _ => (any_json(), names.clone()),
    }
}

fn convert_object(doc: &Doc, node: &Map<String, Value>, hint: &str, names: &Names) -> Converted {
    if let Some(reference) = node.get("$ref").and_then(Value::as_str) {
        return convert_ref(doc, reference, names);
    }
    if let Some(branches) = node
        .get("oneOf")
        .or_else(|| node.get("anyOf"))
        .and_then(Value::as_array)
    {
        return union_of(doc, branches.to_vec(), hint, names);
    }
    if let Some(all) = node.get("allOf").and_then(Value::as_array) {
        return match all.as_slice() {
            [one] => convert(doc, one, hint, names),
            _ => (any_json(), names.clone()),
        };
    }
    match node.get("type") {
        Some(Value::Array(types)) => {
            let branches = types
                .iter()
                .map(|t| Value::Object(with_key(node, "type", t.clone())))
                .collect();
            union_of(doc, branches, hint, names)
        }
        Some(Value::String(t)) => convert_typed(doc, node, t, hint, names),
        _ => convert_untyped(doc, node, hint, names),
    }
}

/// A node that names no `type`: what its keywords say it is.
fn convert_untyped(doc: &Doc, node: &Map<String, Value>, hint: &str, names: &Names) -> Converted {
    if node.contains_key("properties")
        || node.contains_key("additionalProperties")
        || node.contains_key("patternProperties")
    {
        return convert_typed(doc, node, "object", hint, names);
    }
    if node.contains_key("items") || node.contains_key("prefixItems") {
        return convert_typed(doc, node, "array", hint, names);
    }
    let values: Vec<&Value> = node
        .get("enum")
        .and_then(Value::as_array)
        .map(|values| values.iter().collect())
        .or_else(|| node.get("const").map(|value| vec![value]))
        .unwrap_or_default();
    match values.as_slice() {
        [] => (any_json(), names.clone()),
        values => {
            let branches = values
                .iter()
                .map(|value| json!({ "type": json_type(value) }))
                .collect();
            union_of(doc, branches, hint, names)
        }
    }
}

fn convert_typed(
    doc: &Doc,
    node: &Map<String, Value>,
    kind: &str,
    hint: &str,
    names: &Names,
) -> Converted {
    match kind {
        "null" => (json!("null"), names.clone()),
        "boolean" => (json!("boolean"), names.clone()),
        "integer" => (json!("long"), names.clone()),
        "number" => (json!("double"), names.clone()),
        "string" => (json!("string"), names.clone()),
        "array" => convert_array(doc, node, hint, names),
        "object" => convert_record_or_map(doc, node, hint, names),
        _ => (any_json(), names.clone()),
    }
}

fn convert_array(doc: &Doc, node: &Map<String, Value>, hint: &str, names: &Names) -> Converted {
    let item_hint = format!("{hint}Item");
    let (items, names) = match (node.get("prefixItems"), node.get("items")) {
        (Some(Value::Array(tuple)), _) | (None, Some(Value::Array(tuple))) => {
            union_of(doc, tuple.clone(), &item_hint, names)
        }
        (_, Some(items)) => convert(doc, items, &item_hint, names),
        _ => (any_json(), names.clone()),
    };
    (json!({ "type": "array", "items": items }), names)
}

fn convert_record_or_map(
    doc: &Doc,
    node: &Map<String, Value>,
    hint: &str,
    names: &Names,
) -> Converted {
    match node.get("properties").and_then(Value::as_object) {
        Some(properties) if !properties.is_empty() => {
            let name = node
                .get("title")
                .and_then(Value::as_str)
                .map(avro_name)
                .unwrap_or_else(|| avro_name(hint));
            convert_record(doc, node, properties, &name, names)
        }
        _ => convert_map(doc, node, hint, names),
    }
}

fn convert_map(doc: &Doc, node: &Map<String, Value>, hint: &str, names: &Names) -> Converted {
    let value_hint = format!("{hint}Value");
    let (values, names) = match (
        node.get("additionalProperties"),
        node.get("patternProperties"),
    ) {
        (Some(values @ Value::Object(_)), _) => convert(doc, values, &value_hint, names),
        (_, Some(Value::Object(patterns))) if !patterns.is_empty() => union_of(
            doc,
            patterns.values().cloned().collect(),
            &value_hint,
            names,
        ),
        _ => (any_json(), names.clone()),
    };
    (json!({ "type": "map", "values": values }), names)
}

/// A record of `properties`: defined here, or the record of that name already
/// defined when its fields are the same.
fn convert_record(
    doc: &Doc,
    node: &Map<String, Value>,
    properties: &Map<String, Value>,
    name: &str,
    names: &Names,
) -> Converted {
    let (fields, after) = record_fields(doc, node, properties, names);
    match after.defined.get(name) {
        Some(Some(same)) if *same == fields => (full_name(doc, name).into(), after),
        Some(_) => {
            let unique = unique_name(&after, name);
            define(doc, &unique, fields, after)
        }
        None => define(doc, name, fields, after),
    }
}

fn define(doc: &Doc, name: &str, fields: Value, names: Names) -> Converted {
    let record = json!({
        "type": "record",
        "name": name,
        "namespace": doc.namespace,
        "fields": fields.clone(),
    });
    let defined = names
        .defined
        .into_iter()
        .chain(std::iter::once((name.to_string(), Some(fields))))
        .collect();
    (record, Names { defined, ..names })
}

/// One field: an Avro name, its type (nullable with a `null` default unless
/// the schema requires it), and the JSON name where Avro refused it.
fn field(property: &str, avro: Value, required: bool, before: &[Value]) -> Value {
    let taken: Vec<&str> = before
        .iter()
        .filter_map(|f| f.get("name").and_then(Value::as_str))
        .collect();
    let base = avro_name(property);
    let name = std::iter::once(base.clone())
        .chain((2..).map(|n| format!("{base}_{n}")))
        .find(|candidate| !taken.contains(&candidate.as_str()))
        .expect("an unused name exists");
    let (ty, default) = if required {
        (avro, None)
    } else {
        (nullable(avro), Some(Value::Null))
    };
    let entries = [
        Some(("name".to_string(), Value::String(name.clone()))),
        Some(("type".to_string(), ty)),
        default.map(|d| ("default".to_string(), d)),
        (name != property).then(|| (NAME_ATTR.to_string(), Value::String(property.into()))),
    ];
    Value::Object(entries.into_iter().flatten().collect())
}

/// `T` as a union with `null` first.
fn nullable(avro: Value) -> Value {
    match avro {
        Value::Array(branches) => Value::Array(
            std::iter::once(json!("null"))
                .chain(branches.into_iter().filter(|b| *b != json!("null")))
                .collect(),
        ),
        Value::String(ref s) if s == "null" => avro,
        other => json!(["null", other]),
    }
}

/// A `$ref`: a record is defined once and named after; anything else is
/// converted where it is used, and is any JSON where it refers to itself.
fn convert_ref(doc: &Doc, reference: &str, names: &Names) -> Converted {
    if let Some(name) = names.refs.get(reference) {
        return (full_name(doc, name).into(), names.clone());
    }
    let Some(target) = resolve(doc.root, reference) else {
        return (any_json(), names.clone());
    };
    let key = reference.rsplit('/').next().unwrap_or(reference);
    if is_record(doc, target, &[]) {
        let title = target.get("title").and_then(Value::as_str).unwrap_or(key);
        let name = unique_name(names, &avro_name(title));
        let reserved = reserve(names, reference, &name);
        let properties = target
            .get("properties")
            .and_then(Value::as_object)
            .expect("a record has properties");
        let object = target.as_object().expect("a record is an object");
        let (fields, after) = record_fields(doc, object, properties, &reserved);
        let (record, after) = define(doc, &name, fields, after);
        return (record, close(after, reference));
    }
    if names.open.iter().any(|open| open == reference) {
        return (any_json(), names.clone());
    }
    let open = Names {
        open: push(names.open.clone(), reference.to_string()),
        ..names.clone()
    };
    let (avro, after) = convert(doc, target, &pascal(key), &open);
    (avro, close(after, reference))
}

/// The fields of a record, as `convert_record` makes them, without naming it.
fn record_fields(
    doc: &Doc,
    node: &Map<String, Value>,
    properties: &Map<String, Value>,
    names: &Names,
) -> (Value, Names) {
    let required: Vec<&str> = node
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let (fields, names) = properties.iter().fold(
        (Vec::new(), names.clone()),
        |(fields, names), (property, schema)| {
            let (avro, names) = convert(doc, schema, &pascal(property), &names);
            let field = field(
                property,
                avro,
                required.contains(&property.as_str()),
                &fields,
            );
            (push(fields, field), names)
        },
    );
    (Value::Array(fields), names)
}

fn reserve(names: &Names, reference: &str, name: &str) -> Names {
    Names {
        defined: with_entry(&names.defined, name.to_string(), None),
        refs: with_entry(&names.refs, reference.to_string(), name.to_string()),
        open: push(names.open.clone(), reference.to_string()),
    }
}

fn close(names: Names, reference: &str) -> Names {
    Names {
        open: names.open.into_iter().filter(|r| r != reference).collect(),
        ..names
    }
}

/// Whether a node becomes a record: an object with properties, through
/// `$ref`s (`seen` stops a loop of them).
fn is_record(doc: &Doc, node: &Value, seen: &[&str]) -> bool {
    match node.get("$ref").and_then(Value::as_str) {
        Some(reference) if seen.contains(&reference) => false,
        Some(reference) => resolve(doc.root, reference)
            .is_some_and(|target| is_record(doc, target, &[seen, &[reference]].concat())),
        None => {
            !node.get("oneOf").is_some_and(Value::is_array)
                && !node.get("anyOf").is_some_and(Value::is_array)
                && !node.get("allOf").is_some_and(Value::is_array)
                && matches!(node.get("type"), None | Some(Value::String(_)))
                && node.get("type").is_none_or(|t| t == "object")
                && node
                    .get("properties")
                    .and_then(Value::as_object)
                    .is_some_and(|p| !p.is_empty())
        }
    }
}

/// `#/definitions/X`, `#/$defs/X` or any other local JSON pointer.
fn resolve<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    root.pointer(reference.strip_prefix('#')?)
}

/// The union of some branches: flattened, `null` first, a named type once;
/// any JSON as a whole when Avro cannot hold them, with no record defined
/// inside them kept.
fn union_of(doc: &Doc, branches: Vec<Value>, hint: &str, names: &Names) -> Converted {
    let (converted, after) = branches.iter().enumerate().fold(
        (Vec::new(), names.clone()),
        |(out, names), (i, branch)| {
            let (avro, names) = convert(doc, branch, &format!("{hint}{}", i + 1), &names);
            (push(out, avro), names)
        },
    );
    let flat: Vec<Value> = converted
        .into_iter()
        .flat_map(|avro| match avro {
            Value::Array(inner) => inner,
            other => vec![other],
        })
        .collect();
    let deduped = flat.iter().fold(Vec::<Value>::new(), |kept, branch| {
        if kept.iter().any(|k| same_named(k, branch)) {
            kept
        } else {
            push(kept, branch.clone())
        }
    });
    let ordered: Vec<Value> = deduped
        .iter()
        .filter(|b| **b == json!("null"))
        .chain(deduped.iter().filter(|b| **b != json!("null")))
        .cloned()
        .collect();
    if !avro_can_hold(&ordered) {
        return (any_json(), names.clone());
    }
    match ordered.as_slice() {
        [] => (json!("null"), after),
        [one] => (one.clone(), after),
        _ => (Value::Array(ordered), after),
    }
}

/// Two branches of one named type (a definition and a reference to it), or
/// the same unnamed one.
fn same_named(a: &Value, b: &Value) -> bool {
    match (named(a), named(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// A union Avro takes: no two branches of one kind but named ones of
/// different names, and no any JSON beside anything but `null`.
fn avro_can_hold(branches: &[Value]) -> bool {
    let kinds: Vec<String> = branches.iter().map(kind).collect();
    let unique = kinds
        .iter()
        .enumerate()
        .all(|(i, k)| !kinds[..i].contains(k));
    let json_alone =
        !branches.iter().any(is_any_json) || branches.len() <= 2 && branches[0] == "null";
    unique && json_alone
}

/// What a branch is to Avro's rule of one branch per kind.
fn kind(branch: &Value) -> String {
    named(branch)
        .map(|name| format!("named:{name}"))
        .or_else(|| branch.as_str().map(str::to_string))
        .or_else(|| {
            branch
                .get("type")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default()
}

/// The name of a named type, defined or referred to.
fn named(branch: &Value) -> Option<String> {
    match branch {
        Value::String(name) if name.contains('.') => Some(short(name).to_string()),
        Value::Object(o) if o.get("type").and_then(Value::as_str) == Some("record") => {
            o.get("name").and_then(Value::as_str).map(str::to_string)
        }
        _ => None,
    }
}

fn short(full: &str) -> &str {
    full.rsplit('.').next().unwrap_or(full)
}

fn is_any_json(branch: &Value) -> bool {
    branch.get(JSON_ATTR) == Some(&Value::Bool(true))
}

/// Any JSON value, as its text.
fn any_json() -> Value {
    json!({ "type": "string", JSON_ATTR: true })
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn full_name(doc: &Doc, name: &str) -> String {
    format!("{}.{name}", doc.namespace)
}

/// `name`, or `name_2`, `name_3`, … where it is taken.
fn unique_name(names: &Names, name: &str) -> String {
    std::iter::once(name.to_string())
        .chain((2..).map(|n| format!("{name}_{n}")))
        .find(|candidate| !names.defined.contains_key(candidate))
        .expect("an unused name exists")
}

/// A name Avro takes: `[A-Za-z_][A-Za-z0-9_]*`.
pub fn avro_name(text: &str) -> String {
    let body: String = text
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    match body.chars().next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => body,
        _ => format!("_{body}"),
    }
}

fn pascal(name: &str) -> String {
    name.split(|c: char| !c.is_ascii_alphanumeric())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

fn with_key(node: &Map<String, Value>, key: &str, value: Value) -> Map<String, Value> {
    node.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .chain(std::iter::once((key.to_string(), value)))
        .collect()
}

fn with_entry<V: Clone>(map: &BTreeMap<String, V>, key: String, value: V) -> BTreeMap<String, V> {
    map.clone()
        .into_iter()
        .chain(std::iter::once((key, value)))
        .collect()
}

fn push<T>(items: Vec<T>, item: T) -> Vec<T> {
    items.into_iter().chain(std::iter::once(item)).collect()
}
