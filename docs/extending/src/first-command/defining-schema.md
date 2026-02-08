# Defining the Schema

Every command in Panopticon has a **schema** that declares:

- What **attributes** (inputs) it accepts
- What **results** (outputs) it produces

The schema is validated once when the command is first used, catching configuration errors early rather than at runtime.

## Imports

First, import everything you need from the `extend` module:

```rust
use panopticon_core::extend::*;
use panopticon_core::prelude::*;
```

The `extend` module provides all the types needed to build custom commands:

| Type | Purpose |
|------|---------|
| `CommandSpecBuilder` | Builds the command schema |
| `AttributeSpecBuilder` | Builds individual attribute specifications |
| `TypeDef` | Defines the type of an attribute or result |
| `ScalarType` | Primitive types (String, Number, Bool, etc.) |
| `ReferenceKind` | How template references are handled |
| `ResultKind` | Whether a result is Data or Meta |
| `CommandSchema` | Type alias for the schema tuple |
| `LazyLock` | Re-exported for static initialization |

## The CommandSchema Type

A `CommandSchema` is a type alias for:

```rust
type CommandSchema = (Vec<AttributeSpec<&'static str>>, Vec<ResultSpec<&'static str>>);
```

It's a tuple of attribute specifications and result specifications. We wrap it in a `LazyLock` so validation runs exactly once, the first time the command is used:

```rust
static REVERSE_SPEC: CommandSchema = LazyLock::new(|| {
    // Build and return the schema here
});
```

## Building the Schema

### Starting the Builder

Create a new `CommandSpecBuilder`:

```rust
static REVERSE_SPEC: CommandSchema = LazyLock::new(|| {
    CommandSpecBuilder::new()
        // ... add attributes and results ...
        .build()
});
```

The `build()` method at the end:

1. Validates all attribute and result names against the `NamePolicy`
2. Checks that derived results reference valid attributes (if any)
3. Returns the tuple `(Vec<AttributeSpec>, Vec<ResultSpec>)`

### Adding Attributes with AttributeSpecBuilder

Attributes define the inputs your command accepts. Use `AttributeSpecBuilder` to create them:

```rust
CommandSpecBuilder::new()
    .attribute(
        AttributeSpecBuilder::new("input", TypeDef::Scalar(ScalarType::String))
            .required()
            .hint("String to reverse (supports Tera template substitution)")
            .reference(ReferenceKind::StaticTeraTemplate)
            .build(),
    )
    // ...
```

Let's examine each method:

#### `AttributeSpecBuilder::new(name, type_def)`

Creates a new attribute builder with:

- **name**: The attribute name (must follow `NamePolicy` - alphanumeric and underscores only)
- **type_def**: The expected type of the attribute value

Common `TypeDef` variants:

```rust
// Primitive types
TypeDef::Scalar(ScalarType::String)
TypeDef::Scalar(ScalarType::Number)
TypeDef::Scalar(ScalarType::Bool)

// Tabular data (DataFrames)
TypeDef::Tabular

// Arrays
TypeDef::ArrayOf(Box::new(TypeDef::Scalar(ScalarType::String)))

// Objects with specific fields (covered in Spec System docs)
TypeDef::ObjectOf { fields: vec![...] }
```

#### `.required()`

Marks the attribute as required. The pipeline will fail validation if this attribute is not provided:

```rust
AttributeSpecBuilder::new("input", TypeDef::Scalar(ScalarType::String))
    .required()  // Pipeline fails if "input" is missing
```

If you don't call `.required()`, the attribute is optional.

#### `.hint(description)`

Provides a human-readable description. This appears in documentation and error messages:

```rust
.hint("String to reverse (supports Tera template substitution)")
```

#### `.reference(kind)`

Specifies how the attribute handles references to other values in the store:

```rust
.reference(ReferenceKind::StaticTeraTemplate)
```

The `ReferenceKind` variants are:

| Kind | Description |
|------|-------------|
| `StaticTeraTemplate` | Value can contain Tera templates like `{{ ns.value }}` |
| `RuntimeTeraTemplate` | Treated as a Tera template at runtime (for conditions) |
| `StorePath` | Value is a direct reference to a store path |
| `Unsupported` | No reference resolution (default) |

For our `ReverseCommand`, we use `StaticTeraTemplate` so users can write:

```rust
// The input can reference values from other namespaces
let attrs = ObjectBuilder::new()
    .insert("input", "{{ inputs.greeting }}")
    .build_hashmap();
```

#### `.default_value(scalar)`

Provides a default value if the attribute is not specified:

```rust
AttributeSpecBuilder::new("separator", TypeDef::Scalar(ScalarType::String))
    .default_value(ScalarValue::String(",".to_string()))
    .build()
```

#### `.build()`

Finalizes the `AttributeSpec`:

```rust
.build()  // Returns AttributeSpec<&'static str>
```

### Adding Results

Results define what outputs your command produces. Use `fixed_result()` for known output names:

```rust
CommandSpecBuilder::new()
    .attribute(/* ... */)
    .fixed_result(
        "reversed",                           // Result name
        TypeDef::Scalar(ScalarType::String),  // Result type
        Some("The reversed string"),          // Optional hint
        ResultKind::Data,                     // Data or Meta
    )
    .fixed_result(
        "length",
        TypeDef::Scalar(ScalarType::Number),
        Some("Character count of the input"),
        ResultKind::Meta,
    )
    .build()
```

#### Result Names

Result names follow the same `NamePolicy` as attributes:

- Alphanumeric characters and underscores only
- Cannot be reserved names (`item`, `index`)

#### ResultKind

The `ResultKind` indicates how the result should be treated:

| Kind | Purpose |
|------|---------|
| `ResultKind::Data` | Primary output data (the "answer") |
| `ResultKind::Meta` | Metadata about the operation (counts, timing, status) |

This distinction helps when retrieving results - you can fetch just data results or just metadata.

## The Complete Schema

Here's our complete schema for `ReverseCommand`:

```rust
use panopticon_core::extend::*;
use panopticon_core::prelude::*;

static REVERSE_SPEC: CommandSchema = LazyLock::new(|| {
    CommandSpecBuilder::new()
        // Define the "input" attribute
        .attribute(
            AttributeSpecBuilder::new("input", TypeDef::Scalar(ScalarType::String))
                .required()
                .hint("String to reverse (supports Tera template substitution)")
                .reference(ReferenceKind::StaticTeraTemplate)
                .build(),
        )
        // Define the "reversed" result (primary output)
        .fixed_result(
            "reversed",
            TypeDef::Scalar(ScalarType::String),
            Some("The reversed string"),
            ResultKind::Data,
        )
        // Define the "length" result (metadata)
        .fixed_result(
            "length",
            TypeDef::Scalar(ScalarType::Number),
            Some("Character count of the input"),
            ResultKind::Meta,
        )
        .build()
});
```

## Common Results Added Automatically

Every command automatically receives these common results (you don't need to declare them):

| Result | Type | Kind | Description |
|--------|------|------|-------------|
| `status` | String | Meta | Execution status: `success`, `skipped`, `error`, `cancelled` |
| `duration_ms` | Number | Meta | Execution time in milliseconds |

These are injected by the `ExecutableWrapper` that wraps your command.

## Schema Validation

When `build()` is called, Panopticon validates:

1. **Name Policy**: All names must be alphanumeric + underscore, and not reserved
2. **Derived Results**: If using `derived_result()`, the referenced attribute must exist and be `ArrayOf(ObjectOf)`
3. **No Duplicates**: Attribute and result names must be unique

If validation fails, the program panics with a descriptive error message. This happens at initialization time (when the `LazyLock` is first accessed), not at runtime.

## Next Steps

Now that we have our schema defined, let's [implement the traits](./implementing-traits.md) that make the command work.
