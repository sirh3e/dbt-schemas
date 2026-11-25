//! Read a `dbt_project.yml` into its generated type, and say where each of
//! its top-level keys is written.
//!
//!     cargo run --example read_project                      # a built-in sample
//!     cargo run --example read_project -- path/to/dbt_project.yml
//!
//! The YAML is parsed once ([`yaml::load_marked`]); the same tree gives the
//! typed value ([`yaml::to_json`] then [`yaml::from_json`]) and the position
//! of every node ([`yaml::position_of`]), so an editor can point at a key.

use std::path::{Path, PathBuf};

use dbt_schemas::generated::dbt_project::DbtProject;
use dbt_schemas::{Position, yaml};

const SAMPLE: &str = "\
name: shop
profile: warehouse
model-paths: [models]
vars:
  start: '2024-01-01'
";

/// The project's name, and each top-level key with where it starts.
struct Summary {
    name: String,
    model_paths: Vec<String>,
    keys: Vec<(String, Position)>,
}

/// Pure: text in, summary out. Nothing here reads a file or prints.
fn summarize(text: &str, path: &Path) -> dbt_schemas::Result<Summary> {
    let marked = yaml::load_marked(text, path)?;
    let project: DbtProject = yaml::from_json(yaml::to_json(&marked), path)?;
    let keys = yaml::entries(&marked)
        .filter_map(|(key, _)| Some((yaml::as_str(key)?.to_string(), yaml::position_of(key))))
        .collect();
    Ok(Summary {
        name: project.name,
        model_paths: project
            .model_paths
            .map(|paths| paths.iter().cloned().collect())
            .unwrap_or_default(),
        keys,
    })
}

fn render(summary: &Summary) -> String {
    let keys = summary
        .keys
        .iter()
        .map(|(key, at)| {
            format!(
                "  {key} at line {}, column {}",
                at.line + 1,
                at.character + 1
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "project {} (models in {})\n{keys}",
        summary.name,
        summary.model_paths.join(", ")
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // I/O stays here, at the edge: the argument, the file, stdout.
    let (path, text) = match std::env::args_os().nth(1).map(PathBuf::from) {
        Some(path) => {
            let text = std::fs::read_to_string(&path)?;
            (path, text)
        }
        None => (PathBuf::from("dbt_project.yml"), SAMPLE.to_string()),
    };
    println!("{}", render(&summarize(&text, &path)?));
    Ok(())
}
