# Derived Results

Derived results solve a specific problem: what happens when result names aren't known at compile time, but come from user-provided attribute values? Panopticon uses the `LiteralFieldRef` type to make this pattern safe at the type level.

## The Problem

Consider a file loading command where users specify which files to load:

```yaml
files:
  - name: users
    file: data/users.csv
  - name: orders
    file: data/orders.csv
```

The command should produce results named after each file: `users.data`, `users.rows`, `orders.data`, `orders.rows`. But these names come from user input - they're not hardcoded in the schema.

The danger is allowing arbitrary strings as result names. What if someone uses a Tera template?

```yaml
files:
  - name: "{{ user_input }}"  # Could be anything at runtime!
    file: data/malicious.csv
```

If `user_input` resolves to something unexpected, the result name becomes unpredictable, breaking downstream dependencies.

## The Solution: LiteralFieldRef

Panopticon distinguishes between two kinds of fields in `ObjectFields`:

1. **Literal fields** - Values are used as-is, never processed through Tera
2. **Template fields** - Values support Tera substitution at runtime

Only literal fields can provide result names. The type system enforces this:

```rust
// add_literal returns (ObjectFields, LiteralFieldRef)
let (fields, name_ref) = fields.add_literal(
    "name",
    TypeDef::Scalar(ScalarType::String),
    true,
    Some("Identifier for this item"),
);

// add_template returns just ObjectFields (no LiteralFieldRef)
let fields = fields.add_template(
    "file",
    TypeDef::Scalar(ScalarType::String),
    true,
    Some("Path to file (supports Tera)"),
    ReferenceKind::StaticTeraTemplate,
);
```

When you call `derived_result()`, you must provide a `LiteralFieldRef`:

```rust
.derived_result("files", name_ref, Some(TypeDef::Tabular), ResultKind::Data)
```

Since `LiteralFieldRef` can only come from `add_literal()`, the compiler guarantees that result names come from literal (non-template) fields.

## How It Works

The `LiteralFieldRef` is a simple wrapper that carries the field name:

```rust
pub struct LiteralFieldRef<T> {
    name: T,
}

impl<T> LiteralFieldRef<T> {
    pub fn name(&self) -> &T {
        &self.name
    }
}
```

The key is that `ObjectFields::add_literal()` is the **only way** to create a `LiteralFieldRef`:

```rust
impl<T: Into<String> + Clone> ObjectFields<T> {
    // Returns a LiteralFieldRef - can be used for derived results
    pub fn add_literal(
        self,
        name: T,
        ty: TypeDef<T>,
        required: bool,
        hint: Option<T>,
    ) -> (Self, LiteralFieldRef<T>) {
        // ... creates and returns LiteralFieldRef
    }

    // Does NOT return a LiteralFieldRef - cannot be used for derived results
    pub fn add_template(
        self,
        name: T,
        ty: TypeDef<T>,
        required: bool,
        hint: Option<T>,
        reference_kind: ReferenceKind,
    ) -> Self {
        // ... no LiteralFieldRef created
    }
}
```

## Complete Example: FileCommand

Here's how the built-in `FileCommand` uses this pattern:

```rust
static FILECOMMAND_SPEC: CommandSchema = LazyLock::new(|| {
    let (pending, fields) = CommandSpecBuilder::new().array_of_objects(
        "files",
        true,
        Some("Array of {name, file, format} objects to read"),
    );

    // "name" is literal - yields LiteralFieldRef
    let (fields, name_ref) = fields.add_literal(
        "name",
        TypeDef::Scalar(ScalarType::String),
        true,
        Some("Identifier for this file in the TabularStore"),
    );

    // "file" is a template - no LiteralFieldRef (paths can use Tera)
    let fields = fields.add_template(
        "file",
        TypeDef::Scalar(ScalarType::String),
        true,
        Some("Path to the file to read (supports tera templates)"),
        ReferenceKind::StaticTeraTemplate,
    );

    // "format" is also a template
    let fields = fields.add_template(
        "format",
        TypeDef::Scalar(ScalarType::String),
        true,
        Some("Format of the file: csv, json, or parquet"),
        ReferenceKind::StaticTeraTemplate,
    );

    pending
        .finalise_attribute(fields)
        .fixed_result("count", TypeDef::Scalar(ScalarType::Number), Some("Number of files loaded"), ResultKind::Meta)
        .fixed_result("total_rows", TypeDef::Scalar(ScalarType::Number), Some("Total rows across files"), ResultKind::Meta)
        // Derived results use name_ref - each file's name becomes a result prefix
        .derived_result("files", name_ref, Some(TypeDef::Tabular), ResultKind::Data)
        .build()
});
```

With this configuration:

```yaml
files:
  - name: users
    file: "{{ data_dir }}/users.csv"  # Template - resolved at runtime
    format: csv
  - name: orders
    file: "{{ data_dir }}/orders.csv"
    format: csv
```

The results are:
- `load.count` (fixed)
- `load.total_rows` (fixed)
- `load.users.data` (derived from literal "users")
- `load.orders.data` (derived from literal "orders")

The `file` paths can use Tera templates, but `name` values are used exactly as written.

## Builder Validation

The `CommandSpecBuilder::build()` method validates derived results at initialization:

```rust
pub fn build(self) -> (Vec<AttributeSpec<T>>, Vec<ResultSpec<T>>) {
    for result in &self.results {
        if let ResultSpec::DerivedFromSingleAttribute { attribute, name_field, .. } = result {
            // 1. Verify the referenced attribute exists
            let attr = self.attributes.iter()
                .find(|a| &a.name == attribute)
                .unwrap_or_else(|| panic!(
                    "Derived result references unknown attribute '{:?}'",
                    attribute
                ));

            // 2. Verify the attribute is ArrayOf(ObjectOf)
            let fields = extract_object_fields(&attr.ty)
                .unwrap_or_else(|| panic!(
                    "Derived result attribute '{:?}' must be ArrayOf(ObjectOf)",
                    attribute
                ));

            // 3. Verify the name field exists in those fields
            let field_name = name_field.name();
            assert!(
                fields.iter().any(|f| &f.name == field_name),
                "Derived result name_field '{:?}' not found in attribute fields",
                field_name,
            );
        }
    }
    // ...
}
```

This catches misconfigurations early:

```rust
// PANIC: "nonexistent" doesn't match any attribute
.derived_result("nonexistent", name_ref, None, ResultKind::Data)

// PANIC: "scalar_attr" is not ArrayOf(ObjectOf)
.attribute(AttributeSpecBuilder::new("scalar_attr", TypeDef::Scalar(ScalarType::String)).build())
.derived_result("scalar_attr", name_ref, None, ResultKind::Data)
```

## Mixed Literal and Template Fields

A single `ObjectFields` can contain both types of fields:

```rust
let (pending, fields) = CommandSpecBuilder::new().array_of_objects(
    "transforms",
    true,
    Some("Array of transform specifications"),
);

// Literal: safe for result names
let (fields, name_ref) = fields.add_literal(
    "name",
    TypeDef::Scalar(ScalarType::String),
    true,
    Some("Transform identifier"),
);

// Template: supports Tera substitution
let fields = fields.add_template(
    "expression",
    TypeDef::Scalar(ScalarType::String),
    true,
    Some("Tera expression to evaluate"),
    ReferenceKind::StaticTeraTemplate,
);

// Another literal
let (fields, desc_ref) = fields.add_literal(
    "description",
    TypeDef::Scalar(ScalarType::String),
    false,
    Some("Human-readable description"),
);

// Can use name_ref or desc_ref for derived results (both are literals)
// Cannot use expression (it's a template - no LiteralFieldRef exists)
```

## Anti-Patterns

### Trying to Use Template Fields for Derived Results

This won't compile - there's no `LiteralFieldRef` for template fields:

```rust
let (pending, fields) = CommandSpecBuilder::new()
    .array_of_objects("items", true, None);

// Only literal fields return LiteralFieldRef
let (fields, name_ref) = fields.add_literal("name", TypeDef::Scalar(ScalarType::String), true, None);

// Template field - no ref returned
let fields = fields.add_template("dynamic_name", TypeDef::Scalar(ScalarType::String), true, None, ReferenceKind::StaticTeraTemplate);

// This won't compile - there's no LiteralFieldRef for "dynamic_name"
// .derived_result("items", ???, None, ResultKind::Data)
```

### Mismatched Attribute Names

```rust
let (pending, fields) = CommandSpecBuilder::new()
    .array_of_objects("things", true, None);
let (fields, name_ref) = fields.add_literal("name", TypeDef::Scalar(ScalarType::String), true, None);

pending
    .finalise_attribute(fields)
    // PANIC at build(): "items" doesn't exist, the attribute is named "things"
    .derived_result("items", name_ref, None, ResultKind::Data)
    .build();
```

### Using Derived Results with Non-Array Attributes

```rust
let (pending, fields) = CommandSpecBuilder::new()
    .array_of_objects("valid_array", true, None);
let (fields, name_ref) = fields.add_literal("name", TypeDef::Scalar(ScalarType::String), true, None);

pending
    .finalise_attribute(fields)
    // Add a scalar attribute
    .attribute(
        AttributeSpecBuilder::new("scalar_attr", TypeDef::Scalar(ScalarType::String))
            .required()
            .build(),
    )
    // PANIC: scalar_attr is not ArrayOf(ObjectOf)
    .derived_result("scalar_attr", name_ref, None, ResultKind::Data)
    .build();
```

## When to Use Derived Results

Use derived results when:

- Users provide an array of named items
- Each item should produce its own namespaced results
- The item names are known at configuration time (not computed at runtime)

Examples:
- **FileCommand**: Each file has a name, produces `{name}.data`, `{name}.rows`
- **AggregateCommand**: Each aggregation has a name, produces `{name}` scalar result
- **ApiCommand**: Each endpoint has a name, produces `{name}.response`, `{name}.status`

Do not use derived results when:
- Result names are truly fixed (use `fixed_result()`)
- Names need to be computed at runtime (reconsider your design)
- You have a single item, not an array (use `fixed_result()`)

## Summary

- `LiteralFieldRef` is the compile-time proof that a field is literal (not a template)
- Only `add_literal()` returns a `LiteralFieldRef`; `add_template()` does not
- `derived_result()` requires a `LiteralFieldRef`, enforcing safety at the type level
- Builder validation catches misconfigurations (wrong attribute name, wrong type) early
- Use derived results for user-provided arrays where each item produces named results
