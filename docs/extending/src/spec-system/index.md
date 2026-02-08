# Spec System Overview

The spec system is the type-safe schema definition layer for Panopticon commands. It defines what attributes a command accepts, what results it produces, and how those relate to each other. The spec system catches configuration errors at compile time and build time, before they can cause runtime failures.

## Why a Spec System?

Pipeline definitions in Panopticon are data-driven (YAML/TOML/JSON), but the commands that execute them are strongly typed Rust code. The spec system bridges this gap by:

1. **Defining schemas** that validate pipeline configurations
2. **Enforcing naming rules** that prevent conflicts with internal identifiers
3. **Tracking reference kinds** so the engine knows which fields contain templates or store paths
4. **Providing compile-time safety** for derived result patterns

## Core Components

The spec system consists of several interconnected types:

| Type | Purpose |
|------|---------|
| [`TypeDef`](./type-defs.md) | Describes the shape of data: Scalar, Tabular, ArrayOf, ObjectOf |
| [`ReferenceKind`](./reference-kinds.md) | Indicates how to evaluate a field: literal, template, or store path |
| [`ObjectFields`](./object-fields.md) | Builder for ObjectOf fields with literal/template distinction |
| [`LiteralFieldRef`](./literal-field-ref.md) | Compile-time proof that a field contains literal data |
| [`NamePolicy`](./name-policy.md) | Validation rules for names: reserved words, forbidden characters |
| [`ResultSpec`](./result-specs.md) | Specification for command outputs: fixed or derived |

## How the Pieces Fit Together

Here is a typical flow for defining a command spec:

```rust
use panopticon_core::extend::*;

// 1. Start the builder
let builder = CommandSpecBuilder::new();

// 2. Define an array-of-objects attribute
let (pending, fields) = builder.array_of_objects(
    "columns",
    true,
    Some("Column definitions")
);

// 3. Add fields using ObjectFields builder
//    - add_literal() returns a LiteralFieldRef
//    - add_template() does not
let (fields, name_ref) = fields.add_literal(
    "name",
    TypeDef::Scalar(ScalarType::String),
    true,
    None
);

let fields = fields.add_template(
    "expression",
    TypeDef::Scalar(ScalarType::String),
    true,
    None,
    ReferenceKind::RuntimeTeraTemplate
);

// 4. Finalize the attribute and add a derived result
let (attributes, results) = pending
    .finalise_attribute(fields)
    .derived_result("columns", name_ref, None, ResultKind::Data)
    .build();
```

The key insight is that `derived_result()` requires a `LiteralFieldRef`, and the **only way** to obtain one is through `add_literal()`. This means the compiler prevents you from using template fields (whose values change at runtime) as result names.

## Compile-Time vs Build-Time vs Runtime

The spec system provides three levels of validation:

### Compile Time

The type system prevents certain categories of errors entirely:

- Template fields cannot be passed to `derived_result()` (no `LiteralFieldRef` available)
- Attribute references must have valid types

### Build Time (spec construction)

When `CommandSpecBuilder::build()` is called, additional validations run:

- Derived results must reference existing attributes
- Referenced attributes must be `ArrayOf(ObjectOf { ... })`
- All names pass `NamePolicy` validation

### Runtime

During pipeline execution, values are validated against their specs:

- Required attributes must be present
- Types must match expectations
- Templates must resolve to valid values

## Example: Complete Command Spec

```rust
use panopticon_core::extend::*;

fn define_transform_spec() -> (Vec<AttributeSpec<&'static str>>, Vec<ResultSpec<&'static str>>) {
    // Start with a simple scalar attribute
    let builder = CommandSpecBuilder::new()
        .attribute(
            AttributeSpecBuilder::new("source", TypeDef::Scalar(ScalarType::String))
                .required()
                .reference(ReferenceKind::StorePath)
                .hint("Store path to source data")
                .build()
        );

    // Add an array-of-objects attribute with mixed fields
    let (pending, fields) = builder.array_of_objects(
        "transforms",
        true,
        Some("Transform specifications")
    );

    // Literal field: will be used for result names
    let (fields, name_ref) = fields.add_literal(
        "name",
        TypeDef::Scalar(ScalarType::String),
        true,
        Some("Output name for this transform")
    );

    // Template field: evaluated at runtime
    let fields = fields.add_template(
        "expression",
        TypeDef::Scalar(ScalarType::String),
        true,
        Some("Tera expression to compute the value"),
        ReferenceKind::RuntimeTeraTemplate
    );

    // Build with fixed and derived results
    pending
        .finalise_attribute(fields)
        .fixed_result(
            "metadata",
            TypeDef::Scalar(ScalarType::Object),
            Some("Transform metadata"),
            ResultKind::Meta
        )
        .derived_result(
            "transforms",  // Source attribute
            name_ref,      // Name comes from this literal field
            None,          // Type inferred at runtime
            ResultKind::Data
        )
        .build()
}
```

## Next Steps

- [Type Definitions](./type-defs.md) - Learn about Scalar, Tabular, ArrayOf, and ObjectOf
- [Reference Kinds](./reference-kinds.md) - Understand when to use each reference type
- [Object Fields](./object-fields.md) - Master the ObjectFields builder pattern
- [LiteralFieldRef](./literal-field-ref.md) - Dive deep into compile-time safety
- [Name Policy](./name-policy.md) - Avoid naming violations
- [Result Specs](./result-specs.md) - Fixed vs derived results
