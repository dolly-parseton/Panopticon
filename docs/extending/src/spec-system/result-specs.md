# Result Specs

The `ResultSpec` enum defines what outputs a command produces. Results can be **fixed** (known at definition time) or **derived** (names determined by input data).

## The ResultSpec Enum

```rust
pub enum ResultSpec<T: Into<String>> {
    Field {
        name: T,
        ty: TypeDef<T>,
        hint: Option<T>,
        kind: ResultKind,
    },
    DerivedFromSingleAttribute {
        attribute: T,
        name_field: LiteralFieldRef<T>,
        ty: Option<TypeDef<T>>,
        kind: ResultKind,
    },
}
```

## ResultKind

Every result has a `ResultKind` indicating its purpose:

```rust
pub enum ResultKind {
    Data,
    Meta,
}
```

### Data Results

`ResultKind::Data` indicates the result contains primary output data:

- Query results
- Transformed datasets
- Computed values

Data results are typically consumed by downstream commands.

### Meta Results

`ResultKind::Meta` indicates the result contains metadata about execution:

- Row counts
- Execution statistics
- Validation summaries

Meta results are often used for logging or debugging rather than data flow.

## Fixed Results (ResultSpec::Field)

Fixed results have names known at command definition time.

### Structure

```rust
ResultSpec::Field {
    name: T,           // Result identifier
    ty: TypeDef<T>,    // Type of the result
    hint: Option<T>,   // Human-readable description
    kind: ResultKind,  // Data or Meta
}
```

### Creating Fixed Results

Use `CommandSpecBuilder::fixed_result()`:

```rust
let builder = CommandSpecBuilder::new()
    .fixed_result(
        "data",
        TypeDef::Tabular,
        Some("Query results"),
        ResultKind::Data
    )
    .fixed_result(
        "row_count",
        TypeDef::Scalar(ScalarType::Number),
        Some("Number of rows returned"),
        ResultKind::Meta
    );
```

### When to Use

Use fixed results when:

- The command always produces the same named outputs
- The number and names of results are constant
- Other commands can depend on specific result names

### Examples

```rust
// A query command with fixed results
CommandSpecBuilder::new()
    .attribute(
        AttributeSpecBuilder::new("query", TypeDef::Scalar(ScalarType::String))
            .required()
            .build()
    )
    .fixed_result("rows", TypeDef::Tabular, None, ResultKind::Data)
    .fixed_result("columns", TypeDef::Scalar(ScalarType::Array), None, ResultKind::Meta)
    .build()
```

## Derived Results (ResultSpec::DerivedFromSingleAttribute)

Derived results have names determined by values in an array attribute.

### Structure

```rust
ResultSpec::DerivedFromSingleAttribute {
    attribute: T,                 // Name of the source attribute
    name_field: LiteralFieldRef<T>,  // Proof of which field provides names
    ty: Option<TypeDef<T>>,       // Type (None = inferred at runtime)
    kind: ResultKind,             // Data or Meta
}
```

### Creating Derived Results

Use `CommandSpecBuilder::derived_result()`:

```rust
let (pending, fields) = CommandSpecBuilder::new()
    .array_of_objects("transforms", true, None);

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

let (attrs, results) = pending
    .finalise_attribute(fields)
    .derived_result(
        "transforms",  // Source attribute
        name_ref,      // LiteralFieldRef for name field
        None,          // Type inferred at runtime
        ResultKind::Data
    )
    .build();
```

### How Derived Results Work

Given this pipeline configuration:

```yaml
my_transform:
  transforms:
    - name: "total"
      expression: "{{ item.a + item.b }}"
    - name: "average"
      expression: "{{ item.sum / item.count }}"
    - name: "maximum"
      expression: "{{ item.values | max }}"
```

The command produces three results:

- `my_transform.total`
- `my_transform.average`
- `my_transform.maximum`

The names come from the `name` field of each object in the `transforms` array.

### When to Use

Use derived results when:

- The number of outputs depends on configuration
- Users define what outputs they want
- Each array element produces a named result

### The LiteralFieldRef Requirement

`derived_result()` requires a `LiteralFieldRef`:

```rust
pub fn derived_result(
    mut self,
    attribute: T,
    name_field: LiteralFieldRef<T>,  // Required proof
    ty: Option<TypeDef<T>>,
    kind: ResultKind,
) -> Self
```

This ensures the name field contains literal values, not templates. See [LiteralFieldRef](./literal-field-ref.md) for details on why this matters.

### Type Inference

The `ty` parameter can be `None` to infer the type at runtime:

```rust
// Type specified: all derived results are tabular
.derived_result("transforms", name_ref, Some(TypeDef::Tabular), ResultKind::Data)

// Type inferred: determined by actual values at runtime
.derived_result("transforms", name_ref, None, ResultKind::Data)
```

## Build-Time Validation

When `build()` is called, the builder validates derived results:

### 1. Attribute Must Exist

```rust
let (pending, fields) = builder.array_of_objects("things", true, None);
let (fields, name_ref) = fields.add_literal("name", ...);

pending
    .finalise_attribute(fields)
    .derived_result("nonexistent", name_ref, None, ResultKind::Data)
    .build();
// Panics: "Derived result references unknown attribute 'nonexistent'"
```

### 2. Attribute Must Be ArrayOf(ObjectOf)

```rust
CommandSpecBuilder::new()
    .attribute(
        AttributeSpecBuilder::new("scalar_attr", TypeDef::Scalar(ScalarType::String))
            .build()
    )
    // ... somehow have a name_ref ...
    .derived_result("scalar_attr", name_ref, None, ResultKind::Data)
    .build();
// Panics: "Derived result attribute 'scalar_attr' must be ArrayOf(ObjectOf)"
```

### 3. Name Field Must Exist in Attribute

```rust
let (pending, fields) = builder.array_of_objects("items", true, None);
let (fields, wrong_ref) = fields.add_literal("key", ...);

// Use wrong_ref with a different attribute
pending
    .finalise_attribute(fields)
    .array_of_objects("other_items", true, None)
    // ... add different fields ...
    .derived_result("other_items", wrong_ref, None, ResultKind::Data)
    .build();
// Panics: "Derived result name_field 'key' not found in attribute 'other_items' fields"
```

## Complete Example

```rust
use panopticon_core::extend::*;

fn define_multi_output_command() -> (
    Vec<AttributeSpec<&'static str>>,
    Vec<ResultSpec<&'static str>>
) {
    let builder = CommandSpecBuilder::new();

    // Simple configuration attribute
    let builder = builder.attribute(
        AttributeSpecBuilder::new("source", TypeDef::Scalar(ScalarType::String))
            .required()
            .reference(ReferenceKind::StorePath)
            .hint("Input data path")
            .build()
    );

    // Array of output specifications
    let (pending, fields) = builder.array_of_objects(
        "outputs",
        true,
        Some("Output specifications")
    );

    // Literal name field - used for derived results
    let (fields, name_ref) = fields.add_literal(
        "name",
        TypeDef::Scalar(ScalarType::String),
        true,
        Some("Output name")
    );

    // Literal format field
    let (fields, _format_ref) = fields.add_literal(
        "format",
        TypeDef::Scalar(ScalarType::String),
        false,
        Some("Output format: json, csv, parquet")
    );

    // Template filter field
    let fields = fields.add_template(
        "filter",
        TypeDef::Scalar(ScalarType::String),
        false,
        Some("Filter expression"),
        ReferenceKind::RuntimeTeraTemplate
    );

    pending
        .finalise_attribute(fields)
        // Fixed metadata result
        .fixed_result(
            "summary",
            TypeDef::Scalar(ScalarType::Object),
            Some("Execution summary with timing and counts"),
            ResultKind::Meta
        )
        // Derived data results - one per output spec
        .derived_result(
            "outputs",
            name_ref,
            Some(TypeDef::Tabular),
            ResultKind::Data
        )
        .build()
}
```

### Usage in Pipeline

```yaml
multi_output:
  source: "load_data.rows"
  outputs:
    - name: "filtered"
      filter: "{{ item.status == 'active' }}"
    - name: "transformed"
      format: "json"
    - name: "aggregated"
      filter: "{{ item.value > 100 }}"
```

This produces:

- `multi_output.summary` (fixed, Meta)
- `multi_output.filtered` (derived, Data)
- `multi_output.transformed` (derived, Data)
- `multi_output.aggregated` (derived, Data)

## Accessing Result Type

The `ResultSpec` provides a helper method:

```rust
impl<T: Into<String>> ResultSpec<T> {
    pub fn type_def(&self) -> Option<&TypeDef<T>> {
        match self {
            ResultSpec::Field { ty, .. } => Some(ty),
            ResultSpec::DerivedFromSingleAttribute { ty, .. } => ty.as_ref(),
        }
    }
}
```

For fixed results, this always returns `Some`. For derived results, it returns `None` if the type is inferred at runtime.

## Fixed vs Derived: Decision Guide

| Scenario | Use Fixed | Use Derived |
|----------|-----------|-------------|
| Command always produces same outputs | Yes | |
| Number of outputs is configuration-driven | | Yes |
| Output names are static | Yes | |
| Output names come from user input | | Yes |
| Need compile-time type checking | Yes | |
| Type depends on runtime data | | Yes (with `ty: None`) |

## Combining Fixed and Derived

A command can have both:

```rust
builder
    // Fixed results (always present)
    .fixed_result("metadata", TypeDef::Scalar(ScalarType::Object), None, ResultKind::Meta)
    .fixed_result("errors", TypeDef::Tabular, None, ResultKind::Meta)
    // Derived results (depend on configuration)
    .derived_result("transforms", name_ref, None, ResultKind::Data)
    .build()
```

This pattern is common for commands that have predictable metadata outputs plus user-defined data outputs.
