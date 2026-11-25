//! Avro schemas of every schema this build has, and documents written with
//! them (feature `avro`).
//!
//! `build.rs` turns each processed JSON Schema into an Avro schema
//! (`build/avro.rs` says how each keyword maps), so a dbt artifact can travel
//! as Avro: a topic, a data file, a table. A document is walked against its
//! schema rather than through serde, so a value is what the JSON said:
//!
//! ```ignore
//! use dbt_schemas::avro;
//!
//! let schema = avro::schema_of(&run_results)?;      // run_results/v6, by its dbt_schema_version
//! let bytes = schema.encode(&run_results)?;         // one Avro datum
//! assert_eq!(schema.decode(&bytes)?, run_results);  // the fields the schema knows
//! let writer = apache_avro::Writer::new(schema.apache(), Vec::new()); // a container file
//! ```
//!
//! What Avro cannot say is said plainly:
//!
//! - **Unknown fields are dropped.** An Avro record has the fields its
//!   schema names; the artifact types let other fields pass, Avro does not.
//! - **A `null` and an absent field are one.** An optional field is
//!   `["null", T]`, and a `null` read back is left out of the object.
//! - **Any JSON is its text:** what the schema leaves open (`config`,
//!   `adapter_response`, `execution_time`) is a `string` of the value's JSON,
//!   marked `"dbt_schemas.json": true`, and read back as the value.
//! - **A number is a number:** a `double` that is whole reads back as an
//!   integer (`3.0` as `3`), as JSON has one kind of number.
//! - **Maps have no order** in Avro, so two encodings of one document may
//!   differ in bytes; read back, they are the same document.

use std::collections::BTreeMap;
use std::sync::{LazyLock, OnceLock};

use apache_avro::types::Value as Avro;
use serde_json::{Map, Number, Value};

/// Every Avro schema this build has: its module path
/// (`run_results/v6`, `manifest/projection/v12`) and its `.avsc` text.
pub use crate::generated::AVRO as ALL;

/// The attribute that marks a `string` holding any JSON value's text.
pub const JSON_ATTR: &str = "dbt_schemas.json";
/// The attribute that keeps a field's JSON name where Avro refused it.
pub const NAME_ATTR: &str = "dbt_schemas.name";

/// Why a document could not be written as Avro, or read back.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AvroError {
    /// This build has no Avro schema of that module path.
    #[error("this build has no Avro schema of {0} (its schema's feature is off)")]
    NotBuilt(String),
    /// The document names no artifact schema to pick one by.
    #[error("the document names no dbt artifact schema (metadata.dbt_schema_version)")]
    NoVersion,
    /// A value does not fit the schema where it is.
    #[error("at {path}: {found} does not fit {expected}")]
    Mismatch {
        path: String,
        expected: String,
        found: String,
    },
    /// A string marked as JSON holds no JSON.
    #[error("at {path}: the JSON text is no JSON: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    /// apache-avro refused the schema, the value or the bytes.
    #[error(transparent)]
    Avro(#[from] Box<apache_avro::Error>),
}

impl From<apache_avro::Error> for AvroError {
    fn from(error: apache_avro::Error) -> Self {
        Self::Avro(Box::new(error))
    }
}

pub type Result<T, E = AvroError> = std::result::Result<T, E>;

/// One Avro schema: its text, its parsed form, and its named records.
pub struct AvroSchema {
    path: &'static str,
    text: &'static str,
    json: Value,
    named: BTreeMap<String, Value>,
    apache: apache_avro::Schema,
}

impl std::fmt::Debug for AvroSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AvroSchema")
            .field("path", &self.path)
            .finish()
    }
}

static PARSED: LazyLock<BTreeMap<&'static str, OnceLock<AvroSchema>>> = LazyLock::new(|| {
    ALL.iter()
        .map(|(path, _)| (*path, OnceLock::new()))
        .collect()
});

/// The Avro schema of a module path (`run_results/v6`,
/// `manifest/projection/v12`, `yaml/latest/dbt_project`), parsed once.
pub fn schema(path: &str) -> Result<&'static AvroSchema> {
    let (path, text) = ALL
        .iter()
        .find(|(p, _)| *p == path)
        .ok_or_else(|| AvroError::NotBuilt(path.to_string()))?;
    Ok(PARSED[path].get_or_init(|| {
        AvroSchema::parse(path, text).expect("a bundled Avro schema parses (tests/avro.rs)")
    }))
}

/// The Avro schema of an artifact, by the schema its
/// `metadata.dbt_schema_version` names. A manifest is its projection, as
/// [`crate::artifacts::AnyManifestProjection`] reads it, where the build has
/// one: dbt writes manifests the complete schema refuses (Fusion's
/// `language: javascript`). `schema("manifest/v12")` is the complete one.
pub fn schema_of(document: &Value) -> Result<&'static AvroSchema> {
    let named = crate::artifacts::sniff(document).ok_or(AvroError::NoVersion)?;
    let module = named.family.replace('-', "_");
    let complete = format!("{module}/v{}", named.version);
    let projection = format!("{module}/projection/v{}", named.version);
    schema(&projection).or_else(|_| schema(&complete))
}

impl AvroSchema {
    /// Parse an `.avsc` this crate wrote (or one written by its rules).
    pub fn parse(path: &'static str, text: &'static str) -> Result<Self> {
        let json: Value = serde_json::from_str(text).map_err(|source| AvroError::Json {
            path: path.to_string(),
            source,
        })?;
        let apache = apache_avro::Schema::parse(&json)?;
        let named = named_records(&json).collect();
        Ok(Self {
            path,
            text,
            json,
            named,
            apache,
        })
    }

    /// The module path it is the Avro schema of.
    pub fn path(&self) -> &'static str {
        self.path
    }

    /// The `.avsc` text.
    pub fn text(&self) -> &'static str {
        self.text
    }

    /// apache-avro's schema, for container files and readers of its own.
    pub fn apache(&self) -> &apache_avro::Schema {
        &self.apache
    }

    /// A JSON document as an Avro value of this schema.
    pub fn to_avro(&self, document: &Value) -> Result<Avro> {
        to_avro(self, document, &self.json, "$")
    }

    /// An Avro value of this schema as the JSON document it stands for.
    pub fn to_json(&self, value: &Avro) -> Result<Value> {
        to_json(self, value, &self.json, "$")
    }

    /// A JSON document as one Avro datum (no header, no schema).
    pub fn encode(&self, document: &Value) -> Result<Vec<u8>> {
        Ok(apache_avro::to_avro_datum(
            &self.apache,
            self.to_avro(document)?,
        )?)
    }

    /// One Avro datum written with this schema, as its JSON document.
    pub fn decode(&self, bytes: &[u8]) -> Result<Value> {
        let value = apache_avro::from_avro_datum(&self.apache, &mut &bytes[..], None)?;
        self.to_json(&value)
    }

    fn resolve<'a>(&'a self, node: &'a Value) -> &'a Value {
        node.as_str()
            .and_then(|name| self.named.get(name))
            .unwrap_or(node)
    }
}

/// Every record of an `.avsc`, by its full name.
fn named_records(node: &Value) -> Box<dyn Iterator<Item = (String, Value)> + '_> {
    match node {
        Value::Array(items) => Box::new(items.iter().flat_map(named_records)),
        Value::Object(o) => {
            let own = (o.get("type").and_then(Value::as_str) == Some("record"))
                .then(|| {
                    let name = o.get("name")?.as_str()?;
                    let full = match o.get("namespace").and_then(Value::as_str) {
                        Some(ns) => format!("{ns}.{name}"),
                        None => name.to_string(),
                    };
                    Some((full, node.clone()))
                })
                .flatten();
            let inner = ["type", "items", "values", "fields"]
                .into_iter()
                .filter_map(|key| o.get(key))
                .flat_map(named_records);
            Box::new(own.into_iter().chain(inner))
        }
        _ => Box::new(std::iter::empty()),
    }
}

fn mismatch(path: &str, expected: &Value, found: &Value) -> AvroError {
    AvroError::Mismatch {
        path: path.to_string(),
        expected: describe(expected),
        found: json_kind(found).to_string(),
    }
}

fn describe(node: &Value) -> String {
    match node {
        Value::String(name) => name.clone(),
        Value::Array(branches) => {
            let names: Vec<String> = branches.iter().map(describe).collect();
            format!("[{}]", names.join(", "))
        }
        Value::Object(o) => o
            .get("name")
            .or_else(|| o.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string(),
        other => other.to_string(),
    }
}

fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(n) if n.is_f64() => "a float",
        Value::Number(_) => "an integer",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

fn is_any_json(node: &Value) -> bool {
    node.get(JSON_ATTR) == Some(&Value::Bool(true))
}

fn type_of(node: &Value) -> &str {
    match node {
        Value::String(t) => t,
        Value::Object(o) => o.get("type").and_then(Value::as_str).unwrap_or(""),
        _ => "",
    }
}

fn to_avro(schema: &AvroSchema, json: &Value, node: &Value, path: &str) -> Result<Avro> {
    let node = schema.resolve(node);
    if let Value::Array(branches) = node {
        let (index, branch) =
            choose(schema, json, branches).ok_or_else(|| mismatch(path, node, json))?;
        let index = u32::try_from(index).expect("a union is small");
        return Ok(Avro::Union(
            index,
            Box::new(to_avro(schema, json, branch, path)?),
        ));
    }
    if is_any_json(node) {
        return Ok(Avro::String(json.to_string()));
    }
    match (type_of(node), json) {
        ("null", Value::Null) => Ok(Avro::Null),
        ("boolean", Value::Bool(b)) => Ok(Avro::Boolean(*b)),
        ("long", Value::Number(n)) => n
            .as_i64()
            .map(Avro::Long)
            .ok_or_else(|| mismatch(path, node, json)),
        ("double", Value::Number(n)) => n
            .as_f64()
            .map(Avro::Double)
            .ok_or_else(|| mismatch(path, node, json)),
        ("string", Value::String(s)) => Ok(Avro::String(s.clone())),
        ("array", Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(i, item)| to_avro(schema, item, &node["items"], &format!("{path}[{i}]")))
            .collect::<Result<_>>()
            .map(Avro::Array),
        ("map", Value::Object(entries)) => entries
            .iter()
            .map(|(key, value)| {
                to_avro(schema, value, &node["values"], &format!("{path}.{key}"))
                    .map(|v| (key.clone(), v))
            })
            .collect::<Result<_>>()
            .map(Avro::Map),
        ("record", Value::Object(object)) => fields(node)
            .map(|field| {
                let (name, key) = field_names(field);
                let value = object.get(key).unwrap_or(&Value::Null);
                to_avro(schema, value, &field["type"], &format!("{path}.{key}"))
                    .map(|v| (name.to_string(), v))
            })
            .collect::<Result<_>>()
            .map(Avro::Record),
        _ => Err(mismatch(path, node, json)),
    }
}

fn fields(record: &Value) -> impl Iterator<Item = &Value> {
    record["fields"].as_array().into_iter().flatten()
}

/// A field's Avro name, and the JSON key it stands for.
fn field_names(field: &Value) -> (&str, &str) {
    let name = field["name"].as_str().unwrap_or_default();
    let key = field.get(NAME_ATTR).and_then(Value::as_str).unwrap_or(name);
    (name, key)
}

/// The union branch a JSON value is written as: the one it fits best, the
/// first of equals.
fn choose<'a>(
    schema: &AvroSchema,
    json: &Value,
    branches: &'a [Value],
) -> Option<(usize, &'a Value)> {
    branches
        .iter()
        .enumerate()
        .filter_map(|(i, branch)| fit(schema, json, branch).map(|score| (score, i, branch)))
        .fold(
            None,
            |best: Option<(i64, usize, &Value)>, candidate| match best {
                Some(b) if b.0 >= candidate.0 => Some(b),
                _ => Some(candidate),
            },
        )
        .map(|(_, i, branch)| (i, branch))
}

/// How well a JSON value fits a branch, if at all: a record by the keys it
/// knows less the keys it does not, and only with every field it requires.
fn fit(schema: &AvroSchema, json: &Value, branch: &Value) -> Option<i64> {
    let branch = schema.resolve(branch);
    if is_any_json(branch) {
        return (!json.is_null()).then_some(-1);
    }
    match (type_of(branch), json) {
        ("null", Value::Null) | ("boolean", Value::Bool(_)) | ("string", Value::String(_)) => {
            Some(0)
        }
        ("array", Value::Array(_)) | ("map", Value::Object(_)) => Some(0),
        ("long", Value::Number(n)) => n.as_i64().map(|_| 1),
        ("double", Value::Number(_)) => Some(0),
        ("record", Value::Object(object)) => {
            let known: Vec<&str> = fields(branch).map(|f| field_names(f).1).collect();
            let complete = fields(branch)
                .all(|f| f.get("default").is_some() || object.contains_key(field_names(f).1));
            let matched = object
                .keys()
                .filter(|k| known.contains(&k.as_str()))
                .count();
            let unknown = object.len() - matched;
            complete.then(|| 1 + matched as i64 - unknown as i64)
        }
        _ => None,
    }
}

fn to_json(schema: &AvroSchema, value: &Avro, node: &Value, path: &str) -> Result<Value> {
    let node = schema.resolve(node);
    let wrong = || AvroError::Mismatch {
        path: path.to_string(),
        expected: describe(node),
        found: format!("{value:?}").chars().take(40).collect(),
    };
    match (value, node) {
        (Avro::Union(index, inner), Value::Array(branches)) => {
            let branch = branches.get(*index as usize).ok_or_else(wrong)?;
            to_json(schema, inner, branch, path)
        }
        (Avro::String(text), node) if is_any_json(node) => {
            serde_json::from_str(text).map_err(|source| AvroError::Json {
                path: path.to_string(),
                source,
            })
        }
        (Avro::Null, _) => Ok(Value::Null),
        (Avro::Boolean(b), _) => Ok(Value::Bool(*b)),
        (Avro::Int(i), _) => Ok(Value::from(*i)),
        (Avro::Long(i), _) => Ok(Value::from(*i)),
        (Avro::Float(f), _) => Ok(number(f64::from(*f))),
        (Avro::Double(f), _) => Ok(number(*f)),
        (Avro::String(s), _) => Ok(Value::String(s.clone())),
        (Avro::Array(items), _) => items
            .iter()
            .enumerate()
            .map(|(i, item)| to_json(schema, item, &node["items"], &format!("{path}[{i}]")))
            .collect::<Result<_>>()
            .map(Value::Array),
        (Avro::Map(entries), _) => entries
            .iter()
            .map(|(key, item)| {
                to_json(schema, item, &node["values"], &format!("{path}.{key}"))
                    .map(|v| (key.clone(), v))
            })
            .collect::<Result<Map<_, _>>>()
            .map(Value::Object),
        (Avro::Record(values), _) => values
            .iter()
            .zip(fields(node))
            .map(|((_, item), field)| {
                let key = field_names(field).1;
                to_json(schema, item, &field["type"], &format!("{path}.{key}"))
                    .map(|v| (key.to_string(), v))
            })
            .filter(|entry| !matches!(entry, Ok((_, Value::Null))))
            .collect::<Result<Map<_, _>>>()
            .map(Value::Object),
        _ => Err(wrong()),
    }
}

/// A `double` as JSON: a whole one as an integer, as JSON has one kind of
/// number and dbt writes `elapsed_time: 3` as often as `3.5`.
fn number(f: f64) -> Value {
    let whole = f.fract() == 0.0 && f.abs() < 9.007_199_254_740_992e15;
    if whole {
        Value::from(f as i64)
    } else {
        Number::from_f64(f).map_or(Value::Null, Value::Number)
    }
}
