# Panopticon (core)

[![Crates.io](https://img.shields.io/crates/v/panopticon-core)](https://crates.io/crates/panopticon-core)
[![License](https://img.shields.io/crates/l/panopticon-core)](https://github.com/dolly-parseton/panopticon/blob/main/LICENSE)
[![CI](https://github.com/dolly-parseton/panopticon/actions/workflows/ci.yml/badge.svg)](https://github.com/dolly-parseton/panopticon/actions/workflows/ci.yml)
[![docs.rs](https://img.shields.io/docsrs/panopticon-core)](https://docs.rs/panopticon-core)

> I wrote this library to encapsulate some required functionality for a series of tools I want to build. This project started with a icky and arguably 'vibe-coded' attempt to build a TUI tool for chaining KQL queries run against Sentinel workspaces (See kql-panopticon). This worked fine at first but fell apart when I started trying to build 'packs', the workflows of 'steps' (now commands) and input parameters. This library provides a way to execute commands in a much safer / observable way and a way to add in new commands. I plan to write commands that implement these traits in other crates and then use the core + extended crates to write more delibrate tooling.

## What does it do?

Panopticon-core is a Rust library for building declarative data pipelines. You define commands, wire them together in namespaces, and execute them through a state-machine pipeline (`Draft` → `Ready` → `Completed`). The type system enforces valid transitions at compile time — you can't execute a pipeline that hasn't been compiled, and you can't add commands to one that's already running.

**Commands** implement three traits: `Descriptor` (schema), `FromAttributes` (construction from key-value pairs), and `Executable` (async execution). Each command declares its expected inputs and outputs through a spec system that validates attribute names, types, and reference kinds. A `CommandSpecBuilder` provides compile-time guarantees — for example, derived result names (where a result's name comes from an attribute value) can only reference fields proven to be literals via an opaque `LiteralFieldRef` type.

**Namespaces** group commands and control execution mode:
- **Once** — commands run sequentially, one time.
- **Iterative** — commands run N times over an array, object keys, or table rows, with `item` and `index` injected into context each iteration.
- **Static** — no execution, just key-value configuration loaded at pipeline start.

During execution, commands read and write to two stores: a **scalar store** (backed by Tera contexts, so values are available in template expressions) and a **tabular store** (Polars DataFrames). Commands can reference other commands' outputs via `StorePath` dot-notation (`namespace.command.field`), and the pipeline resolves dependencies to determine execution order.

The library ships with five built-in commands:
- **file** — load CSV, JSON, or Parquet files into the tabular store.
- **sql** — run SQL queries against tabular data using Polars' SQL context.
- **aggregate** — compute sum, mean, min, max, count, etc. over tabular columns.
- **condition** — evaluate conditional branches using Tera expressions.
- **template** — render Tera templates to files or capture output as a result.

After execution, a `ResultStore` collects all outputs, writes tabular data to disk in the requested format, and separates metadata (status, duration) from data results.

### Examples

`examples/prelude/` covers pipeline usage:
- `aggregate_and_export.rs` — load CSV, run aggregations, export results.
- `when_conditional.rs` — conditionally skip commands using the `when` attribute.
- `pipeline_reuse.rs` — re-enter Draft from Completed to add commands and re-execute.
- `iterate_object_keys.rs` — iterate over object keys in a namespace.
- `template_inheritance.rs` — render Tera templates with inheritance.
- `multi_format_load.rs` — load CSV, JSON, and Parquet in one pipeline.

`examples/extend/` covers implementing new commands:
- `custom_command.rs` — implement a custom command end-to-end (Descriptor, FromAttributes, Executable) and run it in a pipeline.
- `command_spec_safety.rs` — demonstrates the `CommandSpecBuilder` safety mechanisms: `LiteralFieldRef` compile-time proofs, `NamePolicy` violations, and builder validation errors.

## What's next?

* Additional safety checks at compile and lazylock time.
* Narrow the extend API surface, starting with AttributeSpec used in CommandSpecBuilder, need to write a builder or change the attribute function.
* Extend the spec types to support more verbose ResultStore schema (Maybe consider a ser/de approach to result capture using the specs as a schema?)
* Add some serde support for the specs.
* ~~Write actual documentation, lol~~ Generated some documentation, will need to read it and hand re-write some bits but better than nothing for now, left space for handwritten introductions, see github pages for documentation.
* Create a dedicated `panopticon-kql` crate that uses the `panopticon-core` traits. Will likely write two commands, one for Defender XDR (via the Graph API) and one for Sentinel workspaces (basically already done in the previous tool attempts).
* Explore forensic parser integrations, for example a command type that can expose a EVTX file as a TabularValue. Lots of very fun applications for something like this.
* Finish the TUI attempt started in `kql-panopticon` but using this library and pipeline approach. I think the way I've written the API lends itself well to the REPL approach started but I wasn't engaged enough in how the Ratatui library was being used and alot of functionality got conflated in with the UI (so it's not a quick fix, better to focus on deliberate CLIs for specific tasks).

### Changelog
1. v0.1.0 - Initial implementation
2. v0.2.0 - Added in the extensions and services modules. Services are specific bits of functionality I want to allow consumers of this library to extend, that includes PipelineIO and EventHooks right now. `Extensions` is a new type that lives inside `ExecutionContext`, runtime within `Pipeline::<Ready>` during execution. The inner map can contain an Arc of a specific type. The idea is certain commands will want to generate and or consume a shared type, for example a HTTP client (the use case that drove the design of these type). Added tokio_utils CancellationToken as a test of extensions, this is a built in example but the M365 crate client will be a true extension.