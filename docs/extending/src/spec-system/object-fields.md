# Object Fields

The `ObjectFields` builder provides a type-safe way to construct fields for `ObjectOf` type definitions. Its key feature is the distinction between **literal** and **template** fields, which enables compile-time safety for derived results.

## The ObjectFields Builder

```rust
pub struct ObjectFields<T: Into<String>> {
    fields: Vec<FieldSpec<T>>,
}
```

`ObjectFields` is obtained from `CommandSpecBuilder::array_of_objects()` and provides two methods for adding fields:

- `add_literal()` - Returns `(Self, LiteralFieldRef<T>)`
- `add_template()` - Returns `Self`

This asymmetry is intentional and forms the foundation of compile-time safety.

## Creating ObjectFields

You do not create `ObjectFields` directly. Instead, use `CommandSpecBuilder::array_of_objects()`:

```rust
let builder = CommandSpecBuilder::new();

// array_of_objects returns (PendingAttribute, ObjectFields)
let (pending, fields) = builder.array_of_objects(
    "items",      // Attribute name
    true,         // Required
    Some("List of items to process")  // Hint
);

// Now use 'fields' to add field specifications
// Later use 'pending' to finalize and continue building
```

## add_literal: Literal Fields

`add_literal()` creates a field with `ReferenceKind::Unsupported` and returns a `LiteralFieldRef` handle.

### Signature

```rust
pub fn add_literal(
    self,
    name: T,
    ty: TypeDef<T>,
    required: bool,
    hint: Option<T>,
) -> (Self, LiteralFieldRef<T>)
```

### Usage

```rust
let (fields, name_ref) = fields.add_literal(
    "name",
    TypeDef::Scalar(ScalarType::String),
    true,
    Some("Item name - will be used as result key")
);
```

### Return Value

The tuple `(Self, LiteralFieldRef<T>)` gives you:

1. The updated `ObjectFields` builder to continue adding fields
2. A `LiteralFieldRef` proving this field contains literal data

### When to Use

Use `add_literal()` when:

- The field contains a fixed value (not a template)
- The field might be used as a derived result name
- The field should not undergo any template processing

## add_template: Template Fields

`add_template()` creates a field with a specified `ReferenceKind` and does **not** return a `LiteralFieldRef`.

### Signature

```rust
pub fn add_template(
    self,
    name: T,
    ty: TypeDef<T>,
    required: bool,
    hint: Option<T>,
    kind: ReferenceKind,
) -> Self
```

### Usage

```rust
let fields = fields.add_template(
    "expression",
    TypeDef::Scalar(ScalarType::String),
    true,
    Some("Tera expression: {{ item.value * 2 }}"),
    ReferenceKind::RuntimeTeraTemplate
);
```

### Return Value

Returns only `Self` - no `LiteralFieldRef` is produced.

### When to Use

Use `add_template()` when:

- The field contains a Tera template (static or runtime)
- The field is a store path reference
- The field should never be used as a derived result name

## The Critical Difference

The return type difference is the **entire point** of this design:

| Method | Returns | LiteralFieldRef? |
|--------|---------|------------------|
| `add_literal()` | `(Self, LiteralFieldRef<T>)` | Yes |
| `add_template()` | `Self` | No |

Since `derived_result()` requires a `LiteralFieldRef`, and `add_template()` does not produce one, **the compiler prevents template fields from being used as derived result names**.

## Chaining Fields

Both methods consume and return `Self`, enabling fluent chaining:

```rust
let (pending, fields) = builder.array_of_objects("columns", true, None);

// Chain literal and template fields
let (fields, name_ref) = fields.add_literal(
    "name",
    TypeDef::Scalar(ScalarType::String),
    true,
    None
);

let (fields, _alias_ref) = fields.add_literal(
    "alias",
    TypeDef::Scalar(ScalarType::String),
    false,
    None
);

let fields = fields.add_template(
    "source",
    TypeDef::Scalar(ScalarType::String),
    true,
    None,
    ReferenceKind::StaticTeraTemplate
);

let fields = fields.add_template(
    "transform",
    TypeDef::Scalar(ScalarType::String),
    false,
    None,
    ReferenceKind::RuntimeTeraTemplate
);
```

Note: You can collect multiple `LiteralFieldRef` handles, but you only need one for each derived result.

## Building and Validation

Call `build()` to finalize the fields:

```rust
let field_vec: Vec<FieldSpec<T>> = fields.build();
```

`build()` validates all field names against `DEFAULT_NAME_POLICY`:

- Reserved names (`item`, `index`) are rejected
- Forbidden characters are rejected

### What Happens If Validation Fails

```rust
let (fields, _) = fields.add_literal(
    "item",  // Reserved name!
    TypeDef::Scalar(ScalarType::String),
    true,
    None
);

// This panics:
// "NamePolicy violation: field name 'item' is reserved"
fields.build();
```

## Finalizing the Attribute

Use `PendingAttribute::finalise_attribute()` to complete the attribute and return to the main builder:

```rust
let (pending, fields) = builder.array_of_objects("transforms", true, None);

let (fields, name_ref) = fields.add_literal(/* ... */);
let fields = fields.add_template(/* ... */);

// Finalize and continue with the main builder
let builder = pending.finalise_attribute(fields);

// Now you can add more attributes, fixed results, or derived results
let (attrs, results) = builder
    .derived_result("transforms", name_ref, None, ResultKind::Data)
    .build();
```

## Complete Example

```rust
use panopticon_core::extend::*;

fn build_spec() -> (Vec<AttributeSpec<&'static str>>, Vec<ResultSpec<&'static str>>) {
    let builder = CommandSpecBuilder::new();

    // Simple scalar attribute
    let builder = builder.attribute(
        AttributeSpecBuilder::new("output_format", TypeDef::Scalar(ScalarType::String))
            .default_value(ScalarValue::String("json".to_string()))
            .build()
    );

    // Array of objects with mixed literal and template fields
    let (pending, fields) = builder.array_of_objects(
        "computations",
        true,
        Some("Named computations to perform")
    );

    // Literal: name will be used for derived results
    let (fields, name_ref) = fields.add_literal(
        "name",
        TypeDef::Scalar(ScalarType::String),
        true,
        Some("Computation name (becomes result key)")
    );

    // Literal: optional description
    let (fields, _desc_ref) = fields.add_literal(
        "description",
        TypeDef::Scalar(ScalarType::String),
        false,
        Some("Human-readable description")
    );

    // Template: the actual computation
    let fields = fields.add_template(
        "formula",
        TypeDef::Scalar(ScalarType::String),
        true,
        Some("Tera formula: {{ item.a + item.b }}"),
        ReferenceKind::RuntimeTeraTemplate
    );

    // Template: optional filter condition
    let fields = fields.add_template(
        "condition",
        TypeDef::Scalar(ScalarType::String),
        false,
        Some("Filter: {% if item.valid %}"),
        ReferenceKind::RuntimeTeraTemplate
    );

    // Finalize and build
    pending
        .finalise_attribute(fields)
        .fixed_result(
            "summary",
            TypeDef::Scalar(ScalarType::Object),
            Some("Execution summary"),
            ResultKind::Meta
        )
        .derived_result(
            "computations",
            name_ref,
            None,
            ResultKind::Data
        )
        .build()
}
```

## Why Not Just Use FieldSpec Directly?

You could construct `FieldSpec` and `TypeDef::ObjectOf` directly:

```rust
// This works but provides no compile-time safety
let fields = vec![
    FieldSpec {
        name: "name",
        ty: TypeDef::Scalar(ScalarType::String),
        required: true,
        hint: None,
        reference_kind: ReferenceKind::Unsupported,
    },
    FieldSpec {
        name: "value",
        ty: TypeDef::Scalar(ScalarType::String),
        required: true,
        hint: None,
        reference_kind: ReferenceKind::RuntimeTeraTemplate,
    },
];

let typedef = TypeDef::ArrayOf(Box::new(TypeDef::ObjectOf { fields }));
```

The problem: you have no `LiteralFieldRef`, so you cannot use `derived_result()`. The `ObjectFields` builder is the **only** way to obtain a `LiteralFieldRef`, which is the **only** way to use derived results safely.
