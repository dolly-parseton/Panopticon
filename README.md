# Panopticon (core)

[![Crates.io](https://img.shields.io/crates/v/panopticon-core)](https://crates.io/crates/panopticon-core)
[![License](https://img.shields.io/crates/l/panopticon-core)](https://github.com/dolly-parseton/panopticon/blob/main/LICENSE)
[![CI](https://github.com/dolly-parseton/panopticon/actions/workflows/ci.yml/badge.svg)](https://github.com/dolly-parseton/panopticon/actions/workflows/ci.yml)
[![docs.rs](https://img.shields.io/docsrs/panopticon-core)](https://docs.rs/panopticon-core)

## Pipeline state machine

Pipelines follow a typestate pattern that enforces valid transitions at compile time:

```
Pipeline<Draft>  ──compile()──▶  Pipeline<Ready>  ──run()──▶  Pipeline<Running>  ──wait()──▶  Pipeline<Complete>
```

| State        | Purpose                                                                                                             |
| ------------ | ------------------------------------------------------------------------------------------------------------------- |
| **Draft**    | Construction. Define variables, steps, iterations, guards, returns, hooks, and extensions.                          |
| **Ready**    | Validated. `compile()` simulates execution to catch unresolved references, forward references, and missing sources. |
| **Running**  | Executing in a background thread. Poll status, wait for completion, or cancel.                                      |
| **Complete** | Finished. Read back the variable store and named return projections.                                                |

```rust
use panopticon_core::prelude::*;

let mut pipe = Pipeline::default();

pipe.var("name", "world")?;
pipe.step::<SetVar>("greet", params!("name" => "greeting", "value" => Param::template(vec![
    Param::literal("hello, "),
    Param::reference("name"),
])))?;
pipe.returns("output", params!("greeting" => Param::reference("greeting")))?;

let complete = pipe.compile()?.run().wait()?;
let returns = complete.returns();
```

## Data model

The store is built on three types:

- **`Value`** — scalars: `Null`, `Boolean(bool)`, `Integer(i64)`, `Float(f64)`, `Text(String)`.
- **`StoreEntry`** — recursive nodes: `Var { value, ty }`, `Array(Vec<StoreEntry>)`, `Map(HashMap<String, StoreEntry>)`.
- **`Store<StoreEntry>`** — a flat `HashMap<String, StoreEntry>` with controlled mutation and duplicate rejection.

Builder-style handles (`ArrayHandle`, `MapHandle`) allow ergonomic collection construction:

```rust
pipe.array("items")?.push(1)?.push(2)?.push(3)?;
pipe.map("config")?.insert("host", "localhost")?.insert("port", "8080")?;
```

## Operations

Operations implement a single trait:

```rust
pub trait Operation: 'static {
    fn metadata() -> OperationMetadata where Self: Sized;
    fn execute(context: &mut Context) -> Result<(), OperationError>;
}
```

`OperationMetadata` declares inputs, outputs, and required extensions. At execution time, an operation reads inputs and writes outputs through `Context`, which validates types and resolves output names according to `NameSpec` (static, derived from an input value, or derived with a default).

### Built-in operations

| Operation | Description                                                                              |
| --------- | ---------------------------------------------------------------------------------------- |
| `SetVar`  | Set a global store variable with a name derived from the `name` input.                   |
| `Get`     | Index into an array or look up a map key.                                                |
| `Compare` | Compare two values (`eq`, `neq`, `gt`, `gte`, `lt`, `lte`) with cross-numeric promotion. |
| `Coerce`  | Convert between scalar types (`Boolean`, `Integer`, `Float`, `Text`).                    |
| `Dedupe`  | Remove duplicate entries from an array, preserving insertion order.                      |
| `Len`     | Return the length of text (chars), an array, or a map.                                   |

## Execution nodes

Steps are organized into an execution tree with four node types:

| Node          | Description                                                                                                                                                                                                  |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Step**      | Execute a single operation with resolved parameters.                                                                                                                                                         |
| **IterArray** | Iterate over an array source. Each iteration receives `__index` and `__item` bindings in a **cloned store** for isolation. New keys are diffed back into the parent store under `{iter_name}.{index}.{key}`. |
| **IterMap**   | Iterate over a map source. Each iteration receives `__key` and `__value` bindings with the same cloned-store isolation as array iteration.                                                                   |
| **Guard**     | Conditionally execute a body based on a boolean reference. Guards share the parent scope — outputs are visible to subsequent steps.                                                                          |

Nodes can be nested (guards inside iterations, iterations inside guards, etc.):

```rust
pipe.iter_array("loop", IterSource::array("items"), |_index, item, body| {
    body.guard("check", GuardSource::boolean("flag"), |inner| {
        inner.step::<SetVar>("capture", params!(
            "name" => "result",
            "value" => Param::reference(item),
        ))?;
        Ok(())
    })?;
    Ok(())
})?;
```

## Hooks

Hooks observe or intercept pipeline events. There are two callback types:

- **Observer** — read-only access to the event and store. Cannot abort execution.
- **Interceptor** — can return `HookAction::Abort(reason)` to stop the pipeline.

Events include `BeforeStep`, `AfterStep`, `BeforeIteration`, `AfterIteration`, `GuardPassed`, `GuardFailed`, `BeforeReturns`, `AfterReturns`, `Complete`, and `Error`.

### Built-in hooks

| Hook             | Type        | Description                                                                            |
| ---------------- | ----------- | -------------------------------------------------------------------------------------- |
| `Logger`         | Observer    | Formatted event output to any `Write` sink (default: stderr).                          |
| `EventLog`       | Observer    | Structured event capture into `Arc<Mutex<Vec<EventRecord>>>` for post-mortem analysis. |
| `Profiler`       | Observer    | Per-step wall-clock timing with summary table on completion.                           |
| `StepFilter`     | Interceptor | Allow-list or deny-list for step names; aborts on violation.                           |
| `StoreValidator` | Interceptor | Assert presence and type of store keys after steps; aborts on failure.                 |
| `Timeout`        | Interceptor | Abort execution if elapsed time exceeds a limit.                                       |

```rust
pipe.hook(Logger::default());
pipe.hook(Timeout::new(Duration::from_secs(30)));
```

## Extensions

Extensions let operations share services (HTTP clients, database connections, etc.) through a typed, named container. Any type implementing `Extension` (requires `Clone + Send + Sync + 'static`) can be registered at draft time and accessed by operations via `Context::extension::<T>(name)`.

```rust
pipe.extension("my_client", my_http_client);
```

Operations declare required extensions in their metadata and access them during execution.

## Features

| Feature | Description                                                                                                                                  |
| ------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `serde` | Enables `Pipeline<Complete>::deserialize_returns::<T>(name)` to deserialize named return blocks into any `serde::de::DeserializeOwned` type. |

## Examples

| Example                    | Description                                                        |
| -------------------------- | ------------------------------------------------------------------ |
| `pipeline`                 | Full pipeline usage with steps, returns, error cases, and hooks.   |
| `guard`                    | Guard control flow with conditional execution.                     |
| `iter_array`               | Array iteration with cloned-store isolation.                       |
| `iter_map`                 | Map iteration over key-value pairs.                                |
| `deserialize`              | Deserialize returns into Rust types (requires `--features serde`). |
| `extend/store_scalars`     | Store scalar API usage.                                            |
| `extend/store_collections` | Store collection API with `ArrayHandle` and `MapHandle`.           |
| `extend/store_nested`      | Complex nested store structures.                                   |

```sh
cargo run --example pipeline
cargo run --example deserialize --features serde
```

## Changelog

1. **v0.1.0** — Initial implementation.
2. **v0.2.0** — Added extensions and services modules. Extensions live inside `ExecutionContext` and allow operations to share typed state (e.g. HTTP clients) via `Arc`. Added `CancellationToken` as a built-in extension example.
3. **v0.3.0** — Simplified the execution model. Removed Tera, Polars, tabular stores, and file I/O commands. Replaced the three-trait Command system (`Descriptor`/`FromAttributes`/`Executable`) with a single `Operation` trait. Added the hooks system with six built-in hooks. Added compile-time dependency validation. Data processing is now an extension crate concern.
