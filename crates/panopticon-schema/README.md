# panopticon-schema

Strict-schema YAML deserialisation for [`panopticon-core`] pipelines.

## What it does

Converts YAML workflow documents into `panopticon_core` `Pipeline<Draft>` values, and lifts validated `Pipeline<Ready>` values back into YAML. The format is strictly versioned (`version: v1`), uses adjacent tagging throughout, and rejects unknown fields at parse time.

> [!NOTE]
> **Hooks and extensions are not part of the document.** They are a
> host-program concern: the loader produces a bare draft, and the
> caller injects hooks and extensions on it before `compile()`. This is
> deliberate — hooks are closures and extensions are trait objects,
> neither of which is serialisable.

## Usage

```rust
use panopticon_core::extend::SetVar;
use panopticon_schema::{register_ops, Deserialiser};

let yaml = r#"
version: v1
variables:
  target:
    type: text
    value: world
steps:
  - name: build_greeting
    type: step
    op: SetVar
    params:
      name:
        type: literal
        value:
          type: text
          value: greeting
      value:
        type: template
        value:
          - type: literal
            value:
              type: text
              value: "hello, "
          - type: ref
            value: target
returns:
  out:
    greeting:
      type: ref
      value: greeting
"#;

let ops = register_ops!("SetVar" => SetVar);
let draft = Deserialiser::new(ops).from_yaml(yaml)?;
let complete = draft.compile()?.run().wait()?;
# Ok::<_, Box<dyn std::error::Error>>(())
```

The `register_ops!` macro builds a compile-time table of operation name → Rust type, so YAML `op:` references resolve to real types without runtime trait-object dispatch. Operations must implement `panopticon_core::extend::Operation`.

## Round-tripping a ready pipeline

`Serialiser` is the mirror of `Deserialiser` and shares the same `OperationRegistrationMap`. A `Pipeline<Ready>` can be lifted back to YAML or to an in-memory `types::Document`:

```rust
# use panopticon_core::extend::{Param, Pipeline, SetVar};
# use panopticon_schema::{register_ops, Serialiser};
# let mut pipe = Pipeline::new();
# pipe.var("target", "world")?;
# pipe.step::<SetVar>("build_greeting", panopticon_core::extend::params!(
#     "name" => "greeting",
#     "value" => Param::reference("target"),
# ))?;
let ready = pipe.compile()?;
let serialiser = Serialiser::new(register_ops!("SetVar" => SetVar));
let yaml = serialiser.to_yaml(&ready)?;
# Ok::<_, Box<dyn std::error::Error>>(())
```

The round-trip `from_yaml → compile → to_yaml` preserves document structure, though textual formatting may differ (map ordering, quoting, whitespace).

## In-memory construction

For GUI editors, static analysers, and tests that want to skip YAML text entirely, the `types` submodule exposes the schema data model:

```rust,ignore
use panopticon_schema::types::{Document, Node, ParamSpec, VariableSpec, Version};
use panopticon_schema::Deserialiser;

let doc = Document { version: Version::V1, /* ... */ };
let draft = deserialiser.from_document(doc)?;
```

`Deserialiser::from_document` is the in-memory counterpart to `from_yaml`; `Serialiser::to_document` is the mirror for lifting.

## Schema reference

The complete v1 format contract — every field, every variant, every scoping rule — lives in [`SCHEMA_V1.md`](SCHEMA_V1.md).

## License

GPL-3.0-only.

[`panopticon-core`]: https://docs.rs/panopticon-core
