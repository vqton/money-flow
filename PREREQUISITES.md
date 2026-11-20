# PREREQUISITES.md

## Missing Prerequisites
The project at `d:\money-flow` is missing the following required dependencies in `Cargo.toml`. While the Rust and SQLite environments are ready, the project configuration must be updated to include:

- `rust_decimal`
- `serde`
- `uuid`
- `rusqlite`

## Action
To initialize these dependencies, run the following commands in the project root (`d:\money-flow`):

```bash
cargo add rust_decimal
cargo add serde
cargo add uuid
cargo add rusqlite
```

Once these are in place, the project will be ready to meet all requirements defined in the beta plan.
