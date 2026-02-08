# Name Policy

The `NamePolicy` struct enforces naming conventions across the spec system. It prevents conflicts with reserved identifiers and ensures names are safe for use in templates and store paths.

## The NamePolicy Struct

```rust
pub struct NamePolicy {
    pub reserved_names: &'static [&'static str],
    forbidden_regex: Regex,
}
```

A `NamePolicy` defines:

1. **Reserved names** - Identifiers that cannot be used (exact match)
2. **Forbidden pattern** - A regex matching characters that are not allowed

## Default Name Policy

Panopticon provides a default policy:

```rust
pub static DEFAULT_NAME_POLICY: LazyLock<NamePolicy> =
    LazyLock::new(|| NamePolicy::new(&["item", "index"], r"[^a-zA-Z0-9_]"));
```

This policy:

- **Reserves** `item` and `index`
- **Forbids** any character that is not alphanumeric or underscore

## Reserved Names

### Why "item" and "index"?

These names are reserved because they have special meaning in iterative namespaces:

- `item` - Refers to the current element during iteration
- `index` - Refers to the current position (0-based) during iteration

Using these as field or attribute names would shadow the built-in meanings, causing confusing behavior.

### Example Collision

```yaml
# In iterative context, 'item' refers to current element
transform:
  expression: "{{ item.value * 2 }}"  # item = current row

# If you named a field 'item', confusion ensues
columns:
  - item: "foo"  # Shadows the built-in 'item'!
```

### Violation Message

```
NamePolicy violation: attribute name 'item' is reserved
```

## Forbidden Characters

### Why Restrict Characters?

Names appear in multiple contexts:

1. **Store paths** - `namespace.command.result` uses dots as separators
2. **Tera templates** - `{{ field_name }}` requires valid identifiers
3. **YAML keys** - Some characters have special meaning

Restricting to `[a-zA-Z0-9_]` ensures names work everywhere.

### What Is Forbidden?

Any character matching `[^a-zA-Z0-9_]` is forbidden:

| Forbidden | Reason |
|-----------|--------|
| `.` (dot) | Store path separator |
| ` ` (space) | Invalid in most contexts |
| `-` (hyphen) | Can be confused with subtraction in templates |
| `!@#$%^&*` | Special characters in various syntaxes |
| Unicode | Potential encoding issues |

### Violation Message

```
NamePolicy violation: field name 'bad.name' contains forbidden characters (pattern: [^a-zA-Z0-9_])
```

## When Validation Occurs

The `DEFAULT_NAME_POLICY` is checked at several points:

### 1. ObjectFields.build()

```rust
impl<T: Into<String> + Clone> ObjectFields<T> {
    pub fn build(self) -> Vec<FieldSpec<T>> {
        for field in &self.fields {
            DEFAULT_NAME_POLICY.validate(field.name.clone(), "field");
        }
        self.fields
    }
}
```

### 2. CommandSpecBuilder.build()

```rust
pub fn build(self) -> (Vec<AttributeSpec<T>>, Vec<ResultSpec<T>>) {
    let policy = &*DEFAULT_NAME_POLICY;

    for attr in &self.attributes {
        policy.validate(attr.name.clone(), "attribute");
    }

    for result in &self.results {
        match result {
            ResultSpec::Field { name, .. } => {
                policy.validate(name.clone(), "result");
            }
            ResultSpec::DerivedFromSingleAttribute { .. } => {
                // Derived names come from runtime data, not validated here
            }
        }
    }

    // ... rest of build
}
```

## The validate() Method

```rust
impl NamePolicy {
    pub fn validate(&self, name: impl Into<String>, context: &str) {
        let name = name.into();

        if self.reserved_names.contains(&name.as_str()) {
            panic!(
                "NamePolicy violation: {} name '{}' is reserved",
                context, name
            );
        }

        if self.forbidden_regex.is_match(&name) {
            panic!(
                "NamePolicy violation: {} name '{}' contains forbidden characters (pattern: {})",
                context,
                name,
                self.forbidden_regex.as_str()
            );
        }
    }
}
```

The `context` parameter produces helpful error messages:

- `"NamePolicy violation: attribute name 'item' is reserved"`
- `"NamePolicy violation: field name 'bad.name' contains forbidden characters"`
- `"NamePolicy violation: result name 'my field' contains forbidden characters"`

## Examples of Valid Names

```rust
// All of these pass validation
DEFAULT_NAME_POLICY.validate("data", "result");        // OK
DEFAULT_NAME_POLICY.validate("output_path", "field");  // OK
DEFAULT_NAME_POLICY.validate("Column2", "attribute");  // OK
DEFAULT_NAME_POLICY.validate("myValue", "field");      // OK
DEFAULT_NAME_POLICY.validate("X", "attribute");        // OK
DEFAULT_NAME_POLICY.validate("a1b2c3", "field");       // OK
```

## Examples of Invalid Names

### Reserved Names

```rust
// Panics: "NamePolicy violation: attribute name 'item' is reserved"
DEFAULT_NAME_POLICY.validate("item", "attribute");

// Panics: "NamePolicy violation: field name 'index' is reserved"
DEFAULT_NAME_POLICY.validate("index", "field");
```

### Forbidden Characters

```rust
// Panics: contains forbidden characters
DEFAULT_NAME_POLICY.validate("my field", "attribute");   // space
DEFAULT_NAME_POLICY.validate("store.path", "result");    // dot
DEFAULT_NAME_POLICY.validate("field-name", "field");     // hyphen
DEFAULT_NAME_POLICY.validate("data!", "attribute");      // exclamation
DEFAULT_NAME_POLICY.validate("value@2", "field");        // at sign
```

## Catching Violations at Build Time

Here is how violations manifest in practice:

### Attribute Name Violation

```rust
let result = std::panic::catch_unwind(|| {
    CommandSpecBuilder::<&str>::new()
        .attribute(
            AttributeSpecBuilder::new("item", TypeDef::Scalar(ScalarType::String))
                .build()
        )
        .build();
});
// Panics: "NamePolicy violation: attribute name 'item' is reserved"
```

### Field Name Violation

```rust
let result = std::panic::catch_unwind(|| {
    let (pending, fields) = CommandSpecBuilder::new()
        .array_of_objects("columns", true, None);

    let (fields, _) = fields.add_literal(
        "my.field",  // Contains forbidden '.'
        TypeDef::Scalar(ScalarType::String),
        true,
        None
    );

    pending.finalise_attribute(fields).build();
});
// Panics: "NamePolicy violation: field name 'my.field' contains forbidden characters"
```

### Result Name Violation

```rust
let result = std::panic::catch_unwind(|| {
    CommandSpecBuilder::<&str>::new()
        .fixed_result(
            "output data",  // Contains forbidden space
            TypeDef::Tabular,
            None,
            ResultKind::Data
        )
        .build();
});
// Panics: "NamePolicy violation: result name 'output data' contains forbidden characters"
```

## Derived Results and Name Policy

Derived result names come from runtime data, not from the spec. Therefore, `NamePolicy` does **not** validate them at build time:

```rust
ResultSpec::DerivedFromSingleAttribute { .. } => {
    // Derived result names come from runtime data, not spec-defined names
}
```

However, the `LiteralFieldRef` mechanism ensures the names come from a literal field, so they should be predictable strings defined in the pipeline YAML.

## Custom Name Policy

You can create a custom `NamePolicy` for specialized validation:

```rust
let strict_policy = NamePolicy::new(
    &["item", "index", "self", "this", "parent"],  // More reserved words
    r"[^a-z_]"  // Only lowercase letters and underscores
);

strict_policy.validate("my_field", "field");  // OK
strict_policy.validate("MyField", "field");   // Panics: uppercase forbidden
strict_policy.validate("self", "attribute");  // Panics: reserved
```

However, the default policy is used throughout the standard builders. Custom policies would require building specs manually.

## Best Practices

### DO

- Use `snake_case` for names: `output_path`, `column_name`, `data_source`
- Keep names descriptive but concise
- Use alphabetic characters primarily, numbers sparingly

### DO NOT

- Use reserved words (`item`, `index`)
- Include dots (they are path separators)
- Include spaces or hyphens
- Use special characters
- Start names with numbers (valid but unconventional)

### Naming Examples

| Good | Bad | Reason |
|------|-----|--------|
| `output_file` | `output.file` | Dot is forbidden |
| `column_name` | `column-name` | Hyphen is forbidden |
| `data_source` | `data source` | Space is forbidden |
| `row_count` | `item` | Reserved word |
| `position` | `index` | Reserved word |
