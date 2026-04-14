# Panopticon

[![License](https://img.shields.io/crates/l/panopticon-core)](https://github.com/dolly-parseton/panopticon/blob/main/LICENSE)
[![CI](https://github.com/dolly-parseton/panopticon/actions/workflows/ci.yml/badge.svg)](https://github.com/dolly-parseton/panopticon/actions/workflows/ci.yml)

A typestate pipeline engine for Rust, with compile-time validation, a hooks system, and a typed extension container. Panopticon is organised as a Cargo workspace so the execution engine, the document format, and domain-specific operation packs can evolve independently.

## Workspace crates

| Crate | Version | Description |
| ----- | ------- | ----------- |
| [`panopticon-core`](crates/panopticon-core) | [![Crates.io](https://img.shields.io/crates/v/panopticon-core)](https://crates.io/crates/panopticon-core) [![docs.rs](https://img.shields.io/docsrs/panopticon-core)](https://docs.rs/panopticon-core) | The pipeline engine: typestate state machine, store, operations, hooks, and extensions. |
| [`panopticon-schema`](crates/panopticon-schema) | [![Crates.io](https://img.shields.io/crates/v/panopticon-schema)](https://crates.io/crates/panopticon-schema) [![docs.rs](https://img.shields.io/docsrs/panopticon-schema)](https://docs.rs/panopticon-schema) | Strict-schema YAML (`version: v1`) deserialisation into `Pipeline<Draft>`, with round-trip serialisation back to YAML. |
| `panopticon-m365` | _unpublished_ | Experimental operation pack for Microsoft 365 / Graph data sources. Not part of the published workspace yet. |

## Getting started

Start with [`panopticon-core`](crates/panopticon-core/README.md) for the engine model (states, store, operations, hooks, extensions) and runnable examples. If you want to author pipelines as YAML documents rather than Rust code, see [`panopticon-schema`](crates/panopticon-schema/README.md) and its [`SCHEMA_V1.md`](crates/panopticon-schema/SCHEMA_V1.md) contract.

```sh
cargo run -p panopticon-core --example pipeline
cargo run -p panopticon-schema --example load_yaml
```

## License

GPL-3.0-only. See [LICENSE](LICENSE).
