//! Rust types for dbt's files, generated from dbt-labs' own JSON Schemas.
//!
//! - [`generated`]: typify types of every schema dbt Labs publishes, each
//!   behind a Cargo feature: the YAML file schemas of every variant
//!   (`yaml::<variant>::<file>`) and every version of the JSON artifacts
//!   (`<family>::v<N>`, and each manifest's projection). They mirror the
//!   upstream formats and nothing else: map them into your own types as
//!   soon as they are read.
//! - [`artifacts`]: the artifact families, `Any<Family>` to read a document
//!   as the version it names, and the `$id` of every schema.
//! - [`dbt`]: the dbt releases, the schemas each writes (`dbt::release("1.7.14")`), and
//!   the `dbt-<major>-<minor>` features that build them.
//! - [`yaml`]: dbt YAML files parsed once with saphyr, into a
//!   [`serde_json::Value`] the generated types deserialize from, with the
//!   position of every node.
//! - [`schema_keys`] and [`schema_walk`]: the property names and values the
//!   schemas allow, at the top or along any YAML path, for completion.
//! - [`processed`]: the processed schemas as text, and the latest YAML ones parsed.
//! - `avro` (feature `avro`): every schema built as an Avro schema, and
//!   documents encoded and decoded with them.
//!
//! The schemas are the upstream repositories, git submodules under
//! `schemas/upstream/`; `build.rs` preprocesses them (`build/preprocess.rs`)
//! and generates the types with typify at build time. See the README, and
//! `examples/` for runnable programs that use each module.

pub mod artifacts;
#[cfg(feature = "avro")]
pub mod avro;
pub mod dbt;
mod error;
pub mod generated;
pub mod processed;
#[cfg(feature = "yaml-latest")]
pub mod schema_keys;
#[cfg(feature = "yaml-latest")]
pub mod schema_walk;
pub mod yaml;

pub use error::Error;
pub use yaml::{Position, Span};

pub type Result<T, E = Error> = std::result::Result<T, E>;
