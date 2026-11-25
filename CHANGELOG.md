# Changelog

All notable changes to dbt-schemas, the Rust types of dbt's YAML files
and JSON artifacts.

<!-- git-cliff: end of header -->
## [0.1.1] - 2026-11-25

### 🚀 Features

- *(schemas)* Every variant and version from upstream submodules, released automatically
- *(sync)* Follow an upstream change without a release when no schema changed
- *(dbt)* A feature per dbt release, and the releases at run time
- *(dbt)* The default features are dbt-latest's, and a root is its newest version built

### ⚙️ Miscellaneous Tasks

- Generate the types in build.rs, and add git-cliff, hooks and a justfile
- Actions/checkout v5, which runs on Node.js 24
## [0.1.0] - 2026-11-25

### 🚀 Features

- Dbt's YAML and artifact schemas as Rust types
