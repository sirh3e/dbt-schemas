//! What a properties file (`models/*.yml`) may hold at a YAML path, as an
//! editor's completion would offer it.
//!
//!     cargo run --example complete                         # models[].columns[]
//!     cargo run --example complete -- models '[]' config
//!
//! The path is a list of [`Step`]s: a key, or `[]` for "an item of this
//! list". The keys come from the same processed schema the generated types
//! are compiled from, so nothing is offered that the types would refuse.

use dbt_schemas::schema_walk::{self, Step, ValueShape};

fn step(word: &str) -> Step<'_> {
    match word {
        "[]" => Step::Item,
        key => Step::Key(key),
    }
}

fn shape(shape: ValueShape) -> &'static str {
    match shape {
        ValueShape::Scalar => "",
        ValueShape::Mapping => ":",
        ValueShape::List => "[]",
    }
}

fn first_line(doc: &str) -> &str {
    doc.lines().next().unwrap_or_default()
}

fn main() {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let words = if words.is_empty() {
        ["models", "[]", "columns", "[]"].map(String::from).to_vec()
    } else {
        words
    };
    let path: Vec<Step<'_>> = words.iter().map(|w| step(w)).collect();

    let found = schema_walk::properties_keys_at(&path);
    let keys: Vec<(String, &str)> = found
        .iter()
        .map(|key| {
            (
                format!("{}{}", key.name, shape(key.shape)),
                first_line(&key.doc),
            )
        })
        .collect();
    let width = keys.iter().map(|(label, _)| label.len()).max().unwrap_or(0);
    keys.iter()
        .map(|(label, doc)| format!("{label:<width$}  {doc}").trim_end().to_string())
        .for_each(|line| println!("{line}"));
}
