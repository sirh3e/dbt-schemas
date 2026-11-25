//! Rust types for dbt's files, generated from dbt-labs' own JSON Schemas.
//!
//! - [`generated`]: typify types of the four YAML file schemas
//!   (`dbt_project.yml`, properties `.yml` files, `packages.yml`,
//!   `selectors.yml`) and of the four JSON artifacts (`run_results.json`,
//!   `sources.json`, `catalog.json`, and a projection of `manifest.json`).
//!   They mirror the upstream formats and nothing else: map them into your
//!   own types as soon as they are read.
//! - [`artifacts`]: the artifact modules and the `$id` each schema has.
//! - [`yaml`]: dbt YAML files parsed once with saphyr, into a
//!   [`serde_json::Value`] the generated types deserialize from, with the
//!   position of every node.
//! - [`schema_keys`] and [`schema_walk`]: the property names and values the
//!   schemas allow, at the top or along any YAML path, for completion.
//! - [`processed`]: the processed YAML schemas as text and parsed.
//!
//! The schemas are vendored pristine under `schemas/` and preprocessed for
//! typify by `schemas/preprocess.py`; see the README.

pub mod artifacts;
mod error;
pub mod generated;
pub mod processed;
pub mod schema_keys;
pub mod schema_walk;
pub mod yaml;

pub use error::Error;
pub use yaml::{Position, Span};

pub type Result<T, E = Error> = std::result::Result<T, E>;
