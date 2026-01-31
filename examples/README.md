# Examples

## prelude — pipeline usage

These examples show how to build and run pipelines using the built-in commands.

- **aggregate_and_export** — Loads a CSV file, runs sum/mean/count aggregations over its columns, and exports the results to disk.
- **when_conditional** — Uses the `when` attribute to conditionally skip commands based on a feature flag in a static namespace.
- **pipeline_reuse** — Demonstrates the pipeline state machine: executes a pipeline, transitions back to Draft to add more commands, then re-executes.
- **iterate_object_keys** — Configures an iterative namespace that loops over the keys of a static object, injecting each key as `{{ item }}` in command templates.
- **template_inheritance** — Loads Tera templates from disk and renders them with variable substitution and template inheritance.
- **multi_format_load** — Loads CSV, JSON, and Parquet files in a single pipeline using the file command.

## extend — implementing new commands

These examples show how to create custom commands and use the spec builder.

- **custom_command** — Walks through implementing a command from scratch: defining a `CommandSchema` with `CommandSpecBuilder`, implementing `Descriptor`, `FromAttributes`, and `Executable`, then running the command in a pipeline with Tera template substitution and `InsertBatch` result writing.
- **command_spec_safety** — Exercises the `CommandSpecBuilder` safety mechanisms. Shows how `LiteralFieldRef` prevents template fields from being used as derived result names at compile time, how `NamePolicy` rejects reserved names and forbidden characters at build time, and how the builder catches invalid derived result references.