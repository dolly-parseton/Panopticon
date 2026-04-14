# Panopticon Workflow Schema — v1

This document is the authoritative reference for the v1 YAML schema consumed by `panopticon-schema`. It is aimed at workflow authors — people who write YAML files that become `panopticon-core` pipelines. Rust API documentation lives on docs.rs; this file is the format contract.

## Contents

- [Document shape](#document-shape)
- [`version`](#version)
- [Adjacent tagging convention](#adjacent-tagging-convention)
- [`variables`](#variables)
- [`steps`](#steps)
  - [Step nodes](#step-nodes)
  - [Iteration nodes](#iteration-nodes)
  - [Guard nodes](#guard-nodes)
- [`params` and `ParamSpec`](#params-and-paramspec)
- [`returns`](#returns)
- [Null spellings](#null-spellings)
- [Strictness guarantees](#strictness-guarantees)
- [Scoping rules](#scoping-rules)
- [Serialising pipelines back to YAML](#serialising-pipelines-back-to-yaml)
- [Limitations](#limitations)

---

## Document shape

Every document is a YAML mapping with this top-level structure:

```yaml
version: v1
variables: { ... }    # optional
steps:     [ ... ]    # optional
returns:   { ... }    # optional
```

All four top-level fields are validated. Unknown top-level fields are a hard error.

---

## `version`

Required. The only accepted value in this revision is `v1`.

```yaml
version: v1
```

Adding a v2 does not break v1 documents — the loader dispatches on this field before parsing the rest of the document, so each version parses into its own struct tree.

---

## Adjacent tagging convention

Every typed value in the schema — literals, variables, parameters, and execution nodes — uses the same `type` + `value` pattern. A typed value is always a YAML mapping with exactly two fields:

```yaml
type: <discriminant>    # names the variant
value: <payload>        # the variant's data, shape depends on the discriminant
```

Unit variants (variants without a payload, like `Null`) omit the `value` field:

```yaml
type: none
```

This applies uniformly to `LiteralValue`, `VariableSpec`, `ParamSpec`, and `Node`. The uniformity is the main readability win: once you learn the pattern, every typed thing in a document reads the same way.

---

## `variables`

Optional. A mapping from variable name to a typed value. Variables are registered against the pipeline's store at draft time and are visible to every step that follows.

### Scalar variables

```yaml
variables:
  greeting:
    type: text
    value: "hello, world"
  count:
    type: integer
    value: 42
  ratio:
    type: float
    value: 3.14
  enabled:
    type: boolean
    value: true
  spare_slot:
    type: none
```

Scalars must use their explicit `type`/`value` pair. Bare YAML scalars (e.g. `count: 42`) are **not** accepted — v1 is strict about type declaration because YAML does not distinguish integer from float on its own.

### Array variables

```yaml
variables:
  tags:
    type: array
    value:
      - type: text
        value: critical
      - type: text
        value: production
      - type: text
        value: database
```

Array elements are themselves typed literals (scalar forms only — no nested arrays or maps inside array elements in v1).

### Map variables

```yaml
variables:
  services:
    type: map
    value:
      primary:
        type: text
        value: "postgres-main"
      replica:
        type: text
        value: "postgres-replica"
```

Map values are also typed scalar literals.

---

## `steps`

Optional. An ordered list of execution nodes. List order becomes pipeline execution order — this is guaranteed regardless of how the YAML parser handles map key ordering elsewhere.

Every node is a mapping with a required `name` field (the execution node identifier, used in error messages and dependency tracking) and a required `type` discriminant. The discriminant selects one of four node shapes:

```yaml
steps:
  - name: <identifier>
    type: step | iter_array | iter_map | guard
    # ...type-specific fields
```

### Step nodes

Invoke a named operation with a map of input parameters.

```yaml
- name: set_greeting
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
```

- `name` — unique execution node identifier
- `op` — operation name, looked up in the `OperationRegistrationMap` passed to the `Deserialiser`
- `params` — optional map of parameter name to `ParamSpec` (see [`params` and `ParamSpec`](#params-and-paramspec))

### Iteration nodes

Run a nested `body` of nodes once per element in a source collection.

**Array iteration:**

```yaml
- name: process_items
  type: iter_array
  source: items
  body:
    - name: capture
      type: step
      op: SetVar
      params:
        name:
          type: literal
          value:
            type: text
            value: current_item
        value:
          type: ref
          value: process_items.__item
```

**Map iteration:**

```yaml
- name: walk_config
  type: iter_map
  source: config
  body:
    - name: capture_value
      type: step
      op: SetVar
      params:
        name:
          type: literal
          value:
            type: text
            value: current_value
        value:
          type: ref
          value: walk_config.__value
```

- `source` — name of an array or map variable in the parent scope
- `body` — nested list of nodes, recursively using the same node schema

**Reserved iteration bindings** are injected into the body's scope by core:

| Node         | Bindings                                     |
|--------------|----------------------------------------------|
| `iter_array` | `{node_name}.__index`, `{node_name}.__item`  |
| `iter_map`   | `{node_name}.__key`, `{node_name}.__value`   |

Reference these with a `type: ref` param from inside the body. They are not visible outside the body.

### Guard nodes

Conditionally execute a `body` of nodes based on a boolean variable.

```yaml
- name: gate
  type: guard
  source: should_proceed
  body:
    - name: produce
      type: step
      op: SetVar
      params:
        name:
          type: literal
          value:
            type: text
            value: gated_output
        value:
          type: literal
          value:
            type: text
            value: "only set when source is true"
```

- `source` — name of a boolean variable in the parent scope
- `body` — nested list of nodes

Unlike iteration nodes, guard bodies **share parent scope** — outputs produced by steps inside the body are visible to steps after the guard.

---

## `params` and `ParamSpec`

Every entry in a step's `params` is a `ParamSpec`. The five variants use the same `type` + `value` shape as literals and variables:

### `literal` — a typed constant value

```yaml
params:
  message:
    type: literal
    value:
      type: text
      value: "hello"
  count:
    type: literal
    value:
      type: integer
      value: 3
  ratio:
    type: literal
    value:
      type: float
      value: 0.5
  enabled:
    type: literal
    value:
      type: boolean
      value: true
  slot:
    type: literal
    value:
      type: none
```

The nested `value` is itself a typed literal — the same four scalar types as variables (`text`, `integer`, `float`, `boolean`) plus `none` for null.

### `ref` — reference to a variable or a step output

```yaml
params:
  value:
    type: ref
    value: greeting
  prior:
    type: ref
    value: dedupe_raw.result
```

References are resolved at `compile()` time. See [Scoping rules](#scoping-rules) for how step outputs are keyed.

### `template` — string concatenation

```yaml
params:
  message:
    type: template
    value:
      - type: literal
        value:
          type: text
          value: "count is "
      - type: ref
        value: count_text
      - type: literal
        value:
          type: text
          value: "."
```

Each element is itself a `ParamSpec`. Core resolves them at execution time and stringifies the result of each into a single `text` value.

### `array` — parameter-value array

```yaml
params:
  items:
    type: array
    value:
      - type: literal
        value:
          type: integer
          value: 1
      - type: ref
        value: x
      - type: literal
        value:
          type: integer
          value: 3
```

Used by operations that take array-shaped parameters. Each element is itself a `ParamSpec`.

### `map` — parameter-value map

```yaml
params:
  fields:
    type: map
    value:
      signin:
        type: ref
        value: current_item
      user:
        type: ref
        value: entra_user
```

Used by operations that take map-shaped parameters. Each value is itself a `ParamSpec`.

---

## `returns`

Optional. A mapping from return-block name to a mapping of projection name to `ParamSpec`.

```yaml
returns:
  summary:
    greeting:
      type: ref
      value: greeting
    item_count:
      type: ref
      value: count_unique.result
  debug:
    raw:
      type: ref
      value: raw_tags
```

Each top-level key defines one return block. At the end of execution, each return block's projections are resolved and stored under the key `{block_name}.{projection_name}` in the `Complete` pipeline's returns store.

Return blocks are the primary surface for "workflow output". Values that should be visible to the host program after `compile().run().wait()` must be referenced from a return block.

---

## Null spellings

The `Null` variant of `LiteralValue` and `VariableSpec` accepts four spellings in the `type` field, all equivalent:

| Spelling            | Example              | Notes                                           |
|---------------------|----------------------|-------------------------------------------------|
| `none` (primary)    | `type: none`         | Recommended. Unambiguous, no quoting needed.    |
| `null_value`        | `type: null_value`   | Explicit alias, also unambiguous.               |
| `null` (quoted)     | `type: "null"`       | Matches core's type name. Quotes force string.  |
| `null` (bare)       | `type: null`         | Also works — YAML null is coerced transparently. |

All four land on the same `LiteralValue::Null` / `VariableSpec::Null` variant and lower to `panopticon_core::extend::Value::Null`. The primary spelling `none` is recommended in documentation and examples for readability and because it leaves no ambiguity about what's happening; the aliases exist as conveniences.

---

## Strictness guarantees

The v1 schema enforces the following at parse time:

- **Unknown top-level fields are rejected.** A document with `version: v1` and an unrecognised field like `metadata:` fails to parse with an `unknown field` error.
- **Unknown node types are rejected.** `type: teleport` fails with an `unknown variant` error.
- **Unknown fields inside a node are rejected.** For example, a `type: step` node with an `opcode:` field alongside `op:` fails.
- **Unknown `ParamSpec` forms are rejected.** `type: reference` fails because `ref` is the correct tag.
- **Unknown `VariableSpec` / `LiteralValue` forms are rejected.** `type: string` fails because `text` is the correct tag.
- **Type mismatches in scalar literals are rejected.** `type: integer, value: "hello"` fails because the value is not an integer.

All of the above surface as a `SchemaError::Yaml` carrying the underlying `serde_yaml::Error`, which includes line and column location for parse errors.

Workflow correctness beyond parse-time strictness — reference resolution, type compatibility across steps, duplicate step names, forward references — is validated when the lowered `Pipeline<Draft>` reaches `compile()`. Those errors surface as `SchemaError::Draft`.

---

## Scoping rules

These are behaviours of `panopticon-core` that the schema exposes faithfully. Knowing them is essential for writing workflows that compile and produce the outputs you expect.

### Step output scoping

Not all step outputs land under the same key pattern. It depends on the operation's `OutputScope` metadata.

- **`OutputScope::Global`** — the output is written to a top-level store key derived from an input parameter. `SetVar` is the only built-in that works this way: `SetVar` with `params.name` bound to a literal `"greeting"` writes to the top-level key `greeting`.
- **`OutputScope::Operation`** — the output is written to a step-local key `{step_name}.{output_name}`. Every other built-in core op (`Get`, `Len`, `Compare`, `Coerce`, `Dedupe`) uses this scope. With these ops, the `name` input controls only the *last segment* of the key and defaults to `result` if omitted. A step named `count_unique` using `Len` without a `name` input produces `count_unique.result`.

**Concrete example.** Given:

```yaml
- name: dedupe_raw
  type: step
  op: Dedupe
  params:
    array:
      type: ref
      value: raw_tags
```

the output is at key `dedupe_raw.result`. Downstream steps reference it with a `type: ref, value: dedupe_raw.result` param.

If you write a `name` param bound to `"unique_tags"`, the output moves to `dedupe_raw.unique_tags`. The step name prefix is always present.

### Guard scope sharing

A `guard` body runs in the parent's scope. Steps inside the body write to the same store that surrounds the guard. Outputs produced inside the guard are visible to steps after the guard and to the `returns` block, as long as the guard actually fired at runtime.

### Iteration scope isolation

An `iter_array` or `iter_map` body runs in a **cloned** copy of the parent store, once per iteration. Outputs from one iteration are not visible to the next. After the loop completes, per-iteration outputs are diffed back into the parent store with keys of the form `{iter_name}.{index}.{inner_step_name}.{output_name}`.

For example, given:

```yaml
- name: walk
  type: iter_array
  source: items
  body:
    - name: capture
      type: step
      op: SetVar
      params:
        name:
          type: literal
          value:
            type: text
            value: current_item
        value:
          type: ref
          value: walk.__item
```

run against a three-element `items` array, the parent store afterwards contains keys like `walk.0.current_item`, `walk.1.current_item`, `walk.2.current_item`. **There is no bare `current_item` at the top level.** A `returns` block trying `type: ref, value: current_item` will fail to compile.

If you want aggregate output from an iteration, the current options are:

1. Reference the source array (unchanged) or a top-level variable that was set before the loop.
2. Inspect the `{iter_name}.*` keys from a hook or extension injected by the host program before `compile()`.
3. Wait for a future core op (not yet implemented) that accumulates per-iteration outputs into a shared collection.

---

## Serialising pipelines back to YAML

The schema crate supports the reverse direction in addition to loading: a `Pipeline<Ready>` can be lifted back into a `Document` and serialised to YAML. The entry points are:

- **`Serialiser::to_yaml(&self, &Pipeline<Ready>) -> Result<String, SchemaError>`** — the common path. Builds a document from the ready pipeline and calls `serde_yaml::to_string` on it.
- **`Serialiser::to_document(&self, &Pipeline<Ready>) -> Result<Document, SchemaError>`** — returns the structured document without emitting YAML text. Useful for programmatic inspection, diffing, and round-trip tests.

### Round-trip guarantee

Loading a YAML document, compiling it, and serialising it back produces a **structurally equivalent** document — the two `Document` values compare equal via `PartialEq`. Textual equality (byte-for-byte or even line-for-line) is **not** guaranteed: key ordering, quoting, and whitespace may differ because the round-trip goes through `BTreeMap` and `serde_yaml`'s formatting preferences. If you need textual stability, normalise both documents by round-tripping them through `Document` before comparing.

### Operation registration requirement

`Serialiser` uses the same `OperationRegistrationMap` as `Deserialiser`, but needs the *reverse* lookup (`TypeId → name`) that the forward-only `insert` escape hatch does not populate. Ops must be registered via `OperationRegistrationMap::register::<O>(name)` or the `register_ops!` macro for serialisation to succeed. Attempting to lift a ready pipeline whose step nodes reference an unregistered operation yields `SchemaError::UnregisteredOperation`.

### Scoping in the emitted document

Per-iteration outputs that were diffed back into the parent store with keys like `walk.0.current_item` are **not** re-emitted as standalone variables — serialisation only walks the declared `variables`, `steps`, and `returns` structures. The execution-time iteration fan-out is a runtime artifact, not a document concern.

### Ignored on serialisation

Hooks and extensions attached to the ready pipeline are silently ignored. The v1 schema has no way to express them and the caller is expected to re-inject them on the redrafted pipeline after `from_yaml`.

---

## Limitations

Known limitations of the v1 schema:

- **Hooks and extensions cannot be declared in the document.** They are a host-program concern: the host builds the `Pipeline<Draft>` via the loader, then calls `.hook()` / `.extension()` on it before `compile()`. This is by design — hooks are closures and extensions are trait objects; neither is serialisable.
- **Operation metadata is not validated at parse time.** The schema loader does not know the input specs of the operations it references. Misnamed or missing params pass through `from_yaml` cleanly and are caught only when `compile()` or the operation's runtime `execute()` rejects them.
- **Source spans are not tracked through lowering.** `SchemaError::Yaml` carries line and column information for parse errors, but `SchemaError::UnknownOperation` and `SchemaError::Draft` do not. A step rejected at lowering time is reported by node name, not by source location.
- **Forward-only op registration.** Ops added via `OperationRegistrationMap::insert` (the raw-thunk escape hatch) cannot be serialised because no reverse `TypeId → name` entry is recorded. Use `register::<O>(name)` or `register_ops!` for full round-trip support.
