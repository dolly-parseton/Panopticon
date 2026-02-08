# Type Definitions

The `TypeDef` enum describes the shape of data in Panopticon. Every attribute and result has a `TypeDef` that specifies what kind of data it holds.

## The TypeDef Enum

```rust
pub enum TypeDef<T: Into<String>> {
    Scalar(ScalarType),
    Tabular,
    ArrayOf(Box<TypeDef<T>>),
    ObjectOf { fields: Vec<FieldSpec<T>> },
}
```

The generic parameter `T` allows specs to be defined with either `&'static str` (for compile-time definitions) or `String` (for runtime use). In practice, you will typically use `&'static str`.

## Scalar

`Scalar` represents a single value of a primitive type. The `ScalarType` enum defines the allowed types:

```rust
pub enum ScalarType {
    Null,
    Bool,
    Number,
    String,
    Array,   // JSON array (untyped)
    Object,  // JSON object (untyped)
}
```

### Usage

```rust
// A required string attribute
TypeDef::Scalar(ScalarType::String)

// A boolean flag
TypeDef::Scalar(ScalarType::Bool)

// A numeric value
TypeDef::Scalar(ScalarType::Number)

// An untyped JSON object (flexible but less validated)
TypeDef::Scalar(ScalarType::Object)
```

### When to Use

Use `Scalar` for:

- Simple configuration values (paths, names, flags)
- Store path references
- Template strings
- Any single value that is not tabular data

### Example

```rust
use panopticon_core::extend::*;

let builder = CommandSpecBuilder::new()
    .attribute(
        AttributeSpecBuilder::new("output_path", TypeDef::Scalar(ScalarType::String))
            .required()
            .hint("Path where results will be written")
            .build()
    )
    .attribute(
        AttributeSpecBuilder::new("include_headers", TypeDef::Scalar(ScalarType::Bool))
            .default_value(ScalarValue::Bool(true))
            .build()
    );
```

## Tabular

`Tabular` represents structured data with rows and columns, like a database table or CSV file. This is Panopticon's primary data interchange format.

### Usage

```rust
TypeDef::Tabular
```

### When to Use

Use `Tabular` for:

- Input data to be processed
- Output results from queries
- Any structured dataset

### Characteristics

- Tabular data has a schema (column names and types)
- Can be iterated row by row
- Supports filtering, transformation, and aggregation
- Used with `ReferenceKind::StorePath` to reference data in the store

### Example

```rust
// Result that produces tabular data
builder.fixed_result(
    "data",
    TypeDef::Tabular,
    Some("Query results as tabular data"),
    ResultKind::Data
)
```

## ArrayOf

`ArrayOf` creates a type representing an array of another type. It is recursive, allowing nested structures.

### Usage

```rust
// Array of strings
TypeDef::ArrayOf(Box::new(TypeDef::Scalar(ScalarType::String)))

// Array of objects (common pattern)
TypeDef::ArrayOf(Box::new(TypeDef::ObjectOf { fields: vec![...] }))
```

### When to Use

Use `ArrayOf` for:

- Lists of configuration items
- Multiple input/output specifications
- Any repeated structure

### Important: ArrayOf(ObjectOf) Pattern

The `ArrayOf(ObjectOf { ... })` pattern is special in Panopticon. It is used for:

1. Defining complex attribute structures with multiple named fields
2. Enabling **derived results** where each object in the array produces a named output

This is the **only** structure that supports `derived_result()`.

### Example

```rust
// Manual construction (rarely needed)
let fields = vec![
    FieldSpec {
        name: "name",
        ty: TypeDef::Scalar(ScalarType::String),
        required: true,
        hint: Some("Column name"),
        reference_kind: ReferenceKind::Unsupported,
    },
    FieldSpec {
        name: "type",
        ty: TypeDef::Scalar(ScalarType::String),
        required: false,
        hint: None,
        reference_kind: ReferenceKind::Unsupported,
    },
];

let typedef = TypeDef::ArrayOf(Box::new(TypeDef::ObjectOf { fields }));

// Better: use the builder (see Object Fields chapter)
let (pending, fields) = builder.array_of_objects("columns", true, None);
```

## ObjectOf

`ObjectOf` represents a structured object with named fields. Each field has its own `TypeDef` and metadata.

### Usage

```rust
TypeDef::ObjectOf {
    fields: vec![
        FieldSpec { name: "name", ty: TypeDef::Scalar(ScalarType::String), ... },
        FieldSpec { name: "value", ty: TypeDef::Scalar(ScalarType::Number), ... },
    ]
}
```

### When to Use

Use `ObjectOf` primarily inside `ArrayOf`:

- `ArrayOf(ObjectOf { ... })` for lists of structured items
- Rarely used standalone

### The FieldSpec Structure

Each field in an `ObjectOf` is defined by `FieldSpec`:

```rust
pub struct FieldSpec<T: Into<String>> {
    pub name: T,
    pub ty: TypeDef<T>,
    pub required: bool,
    pub hint: Option<T>,
    pub reference_kind: ReferenceKind,
}
```

| Field | Purpose |
|-------|---------|
| `name` | Field identifier (must pass NamePolicy) |
| `ty` | The type of this field |
| `required` | Whether the field must be present |
| `hint` | Human-readable description |
| `reference_kind` | How to evaluate this field (see [Reference Kinds](./reference-kinds.md)) |

## Type Nesting

Types can be nested to create complex structures:

```rust
// Array of arrays of strings (rarely needed)
TypeDef::ArrayOf(Box::new(
    TypeDef::ArrayOf(Box::new(
        TypeDef::Scalar(ScalarType::String)
    ))
))

// Array of objects containing arrays
let inner_fields = vec![
    FieldSpec {
        name: "tags",
        ty: TypeDef::ArrayOf(Box::new(TypeDef::Scalar(ScalarType::String))),
        required: false,
        hint: None,
        reference_kind: ReferenceKind::Unsupported,
    },
];
TypeDef::ArrayOf(Box::new(TypeDef::ObjectOf { fields: inner_fields }))
```

## Type Conversion

`TypeDef<&'static str>` can be converted to `TypeDef<String>` using `Into`:

```rust
let static_type: TypeDef<&'static str> = TypeDef::Scalar(ScalarType::String);
let owned_type: TypeDef<String> = static_type.into();
```

This conversion is handled automatically by the builder when constructing `CommandSpec`.

## Common Patterns

### Configuration Attribute

```rust
AttributeSpecBuilder::new("config", TypeDef::Scalar(ScalarType::Object))
    .required()
    .hint("JSON configuration object")
    .build()
```

### Store Path Reference

```rust
AttributeSpecBuilder::new("source", TypeDef::Scalar(ScalarType::String))
    .required()
    .reference(ReferenceKind::StorePath)
    .hint("Path to source data in store")
    .build()
```

### Named Transforms

```rust
let (pending, fields) = builder.array_of_objects("transforms", true, None);

let (fields, name_ref) = fields.add_literal(
    "name",
    TypeDef::Scalar(ScalarType::String),
    true,
    None
);

let fields = fields.add_template(
    "expr",
    TypeDef::Scalar(ScalarType::String),
    true,
    None,
    ReferenceKind::RuntimeTeraTemplate
);
```

## What Happens If You Get It Wrong

### Wrong Type at Runtime

If a pipeline provides data that does not match the expected type, validation fails:

```yaml
# Spec expects: TypeDef::Scalar(ScalarType::Number)
my_command:
  count: "not a number"  # Error: expected number, got string
```

### Invalid Derived Result Structure

If you try to create a derived result from an attribute that is not `ArrayOf(ObjectOf)`:

```rust
// This will panic at build time:
// "Derived result attribute 'source' must be ArrayOf(ObjectOf)"
builder
    .attribute(
        AttributeSpecBuilder::new("source", TypeDef::Scalar(ScalarType::String))
            .build()
    )
    .derived_result("source", some_ref, None, ResultKind::Data)
    .build();
```

The spec system catches this error when `build()` is called, before any pipeline execution.
