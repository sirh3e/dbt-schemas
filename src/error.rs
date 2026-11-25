use std::path::PathBuf;

/// What can go wrong reading a dbt file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The text is no YAML, or holds no document.
    #[error("yaml error in {path}: {message}")]
    Yaml { path: PathBuf, message: String },
    /// The YAML does not fit the generated type.
    #[error("{path} does not fit the dbt schema: {source}")]
    Shape {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}
