//! Property names the YAML schemas allow, sorted, for completion. Read from
//! the same processed schemas the generated types are compiled from; for
//! keys at any depth, see [`crate::schema_walk`].

use crate::processed;

fn keys_at(root: &serde_json::Value, path: &[&str]) -> Vec<String> {
    let mut node = root;
    for step in path {
        node = match node.get(step) {
            Some(next) => next,
            None => return Vec::new(),
        };
    }
    node.get("properties")
        .and_then(|p| p.as_object())
        .map(|o| {
            let mut keys: Vec<String> = o.keys().cloned().collect();
            keys.sort();
            keys
        })
        .unwrap_or_default()
}

/// The top-level keys of a properties `.yml` file.
pub fn properties_root_keys() -> Vec<String> {
    keys_at(processed::dbt_yml_files(), &[])
}

/// The keys of an entry of `models:`.
pub fn model_item_keys() -> Vec<String> {
    keys_at(
        processed::dbt_yml_files(),
        &["properties", "models", "items"],
    )
}

/// The keys of an entry of `sources:`.
pub fn source_item_keys() -> Vec<String> {
    keys_at(
        processed::dbt_yml_files(),
        &["properties", "sources", "items"],
    )
}

/// The top-level keys of `dbt_project.yml`.
pub fn project_root_keys() -> Vec<String> {
    keys_at(processed::dbt_project(), &[])
}
