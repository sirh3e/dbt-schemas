//! The marked-YAML helpers, and the YAML types read through them.

use std::path::Path;

use dbt_schemas::generated::{dbt_project::DbtProject, dbt_yml_files::DbtYmlFiles};
use dbt_schemas::{Error, Position, yaml};
use pretty_assertions::assert_eq;

const PROJECT: &str = "name: shop\nprofile: warehouse\nmodel-paths: [models, more]\nvars:\n  start: '2024-01-01'\n  inner:\n    n: 3\n";

#[test]
fn a_project_file_reads_into_its_generated_type() {
    let path = Path::new("dbt_project.yml");
    let marked = yaml::load_marked(PROJECT, path).unwrap();
    let project: DbtProject = yaml::from_json(yaml::to_json(&marked), path).unwrap();
    assert_eq!(project.name, "shop");
    assert_eq!(project.profile.as_deref(), Some("warehouse"));
    let paths: Vec<String> = project.model_paths.unwrap().iter().cloned().collect();
    assert_eq!(paths, ["models", "more"]);
}

#[test]
fn nodes_keep_their_positions() {
    let marked = yaml::load_marked(PROJECT, Path::new("p.yml")).unwrap();
    let vars = yaml::get(&marked, "vars").unwrap();
    let start = yaml::get(vars, "start").unwrap();
    assert_eq!(
        yaml::position_of(start),
        Position {
            line: 4,
            character: 9
        }
    );
    let keys: Vec<&str> = yaml::entries(vars)
        .filter_map(|(k, _)| yaml::as_str(k))
        .collect();
    assert_eq!(keys, ["start", "inner"]);
    let paths = yaml::get(&marked, "model-paths").unwrap();
    assert_eq!(
        yaml::items(paths)
            .filter_map(yaml::as_str)
            .collect::<Vec<_>>(),
        ["models", "more"]
    );
}

#[test]
fn scalars_become_json_values() {
    let marked =
        yaml::load_marked("a: 1\nb: 1.5\nc: true\nd: ~\n1: one\n", Path::new("x.yml")).unwrap();
    assert_eq!(
        yaml::to_json(&marked),
        serde_json::json!({"a": 1, "b": 1.5, "c": true, "d": null, "1": "one"})
    );
}

#[test]
fn a_properties_file_reads_into_its_generated_type() {
    let text = "version: 2\nmodels:\n  - name: orders\n    description: one row per order\n    columns:\n      - name: id\n        description: the key\n";
    let path = Path::new("models/schema.yml");
    let marked = yaml::load_marked(text, path).unwrap();
    let file: DbtYmlFiles = yaml::from_json(yaml::to_json(&marked), path).unwrap();
    let models = file.models;
    assert_eq!(models[0].name, "orders");
}

#[test]
fn what_is_no_yaml_names_its_file_and_place() {
    let err = yaml::load_marked("a: [1, 2\n", Path::new("bad.yml")).unwrap_err();
    assert!(
        matches!(&err, Error::Yaml { path, .. } if path == Path::new("bad.yml")),
        "{err}"
    );
    let (at, message) = yaml::syntax_error("a: b\nc: [1, 2\n").unwrap();
    assert!(at.line >= 1, "{at:?} {message}");
    assert_eq!(yaml::syntax_error("a: b\n"), None);
    let empty = yaml::load_marked("", Path::new("empty.yml")).unwrap_err();
    assert!(empty.to_string().contains("empty yaml document"));
}

#[test]
fn what_does_not_fit_the_schema_is_a_shape_error() {
    let path = Path::new("dbt_project.yml");
    let marked = yaml::load_marked("profile: p\n", path).unwrap();
    let err = yaml::from_json::<DbtProject>(yaml::to_json(&marked), path).unwrap_err();
    assert!(matches!(err, Error::Shape { .. }), "{err}");
}

#[test]
fn the_schema_keys_list_what_completion_offers() {
    assert!(dbt_schemas::schema_keys::project_root_keys().contains(&"model-paths".to_string()));
    assert!(dbt_schemas::schema_keys::model_item_keys().contains(&"columns".to_string()));
    assert!(dbt_schemas::schema_keys::source_item_keys().contains(&"tables".to_string()));
    assert!(dbt_schemas::schema_keys::properties_root_keys().contains(&"models".to_string()));
}

#[test]
fn a_span_runs_from_the_start_of_a_node_to_its_end() {
    let marked = yaml::load_marked("name: shop\n", Path::new("p.yml")).unwrap();
    let span = yaml::span_of(yaml::get(&marked, "name").unwrap());
    assert_eq!(
        span.start,
        Position {
            line: 0,
            character: 6
        }
    );
    assert_eq!(
        span.end,
        Position {
            line: 0,
            character: 10
        }
    );
}
