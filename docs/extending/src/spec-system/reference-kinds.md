# Reference Kinds

The `ReferenceKind` enum tells the Panopticon engine how to interpret and evaluate a field's value. This is critical for dependency tracking, template rendering, and data flow.

## The ReferenceKind Enum

```rust
#[derive(Debug, Clone, PartialEq, Hash, Eq, Default)]
pub enum ReferenceKind {
    StaticTeraTemplate,   // Tera template evaluated before command runs
    RuntimeTeraTemplate,  // Tera template evaluated during execution
    StorePath,            // Direct reference to data in the store
    #[default]
    Unsupported,          // Literal value, no evaluation needed
}
```

## Unsupported (Default)

`Unsupported` means the field contains a literal value that does not need any evaluation or interpretation. The value is used exactly as provided.

### When to Use

- String identifiers (names, labels)
- Boolean flags
- Numeric constants
- Any value that should not be interpreted as a template or reference

### Characteristics

- No template rendering
- No dependency tracking
- Value is passed through unchanged
- **Only `Unsupported` fields can produce `LiteralFieldRef`**

### Example

```rust
// A simple name field - no template processing
let (fields, name_ref) = fields.add_literal(
    "output_name",
    TypeDef::Scalar(ScalarType::String),
    true,
    Some("Name for the output (literal string)")
);
// Note: add_literal() implies ReferenceKind::Unsupported
```

## StaticTeraTemplate

`StaticTeraTemplate` marks a field whose value is a [Tera template](https://keats.github.io/tera/) that will be evaluated **before** the command executes. The template has access to the current namespace context.

### When to Use

- Configuration values that need variable substitution
- Paths that incorporate variables
- Strings that should be computed once at command start

### Characteristics

- Evaluated during command initialization
- Has access to namespace variables
- Result is cached for the command's lifetime
- Dependencies are resolved statically

### Example

```rust
let fields = fields.add_template(
    "output_path",
    TypeDef::Scalar(ScalarType::String),
    true,
    Some("Path with variable substitution: {{ base_path }}/{{ name }}.csv"),
    ReferenceKind::StaticTeraTemplate
);
```

### Template Syntax

```yaml
my_command:
  output_path: "{{ output_dir }}/report_{{ date }}.json"
```

The variables `output_dir` and `date` are resolved from the namespace context before execution.

## RuntimeTeraTemplate

`RuntimeTeraTemplate` marks a field whose value is a Tera template evaluated **during** command execution, potentially multiple times. This is used for expressions that depend on row-level data.

### When to Use

- Computed fields based on input data
- Row-level transformations
- Conditional logic that depends on values from each record

### Characteristics

- Evaluated during iteration over data
- Has access to current row values (typically as `item`)
- May be evaluated many times (once per row)
- Cannot be used for derived result names (dynamic evaluation makes names unpredictable)

### Example

```rust
let fields = fields.add_template(
    "expression",
    TypeDef::Scalar(ScalarType::String),
    true,
    Some("Tera expression evaluated for each row"),
    ReferenceKind::RuntimeTeraTemplate
);
```

### Template Syntax

```yaml
transform:
  columns:
    - name: "full_name"
      expression: "{{ item.first_name }} {{ item.last_name }}"
    - name: "is_adult"
      expression: "{% if item.age >= 18 %}true{% else %}false{% endif %}"
```

The `item` variable contains the current row during iteration.

## StorePath

`StorePath` marks a field whose value is a reference to data in the Panopticon store. The engine uses this to track dependencies between commands.

### When to Use

- Input data references
- References to other command outputs
- Any field that points to stored data

### Characteristics

- Value is interpreted as a store path (e.g., `namespace.command.result`)
- Engine tracks this as a dependency
- Data is loaded from the store when accessed
- Supports both scalar and tabular data

### Example

```rust
AttributeSpecBuilder::new("source", TypeDef::Scalar(ScalarType::String))
    .required()
    .reference(ReferenceKind::StorePath)
    .hint("Store path to input data")
    .build()

// Or for tabular references:
AttributeSpecBuilder::new("data", TypeDef::Tabular)
    .required()
    .reference(ReferenceKind::StorePath)
    .hint("Tabular data from another command")
    .build()
```

### Path Syntax

```yaml
my_command:
  source: "other_namespace.load_data.rows"
```

This creates a dependency: `my_command` depends on `other_namespace.load_data.rows`.

## Reference Kind and LiteralFieldRef

A critical interaction exists between `ReferenceKind` and the `LiteralFieldRef` mechanism:

| ReferenceKind | Can produce LiteralFieldRef? | Reason |
|---------------|------------------------------|--------|
| `Unsupported` | Yes | Value is known at definition time |
| `StaticTeraTemplate` | No | Value depends on namespace variables |
| `RuntimeTeraTemplate` | No | Value depends on row data |
| `StorePath` | No | Value is a reference, not a name |

This is enforced by the `ObjectFields` builder:

- `add_literal()` forces `ReferenceKind::Unsupported` and returns a `LiteralFieldRef`
- `add_template()` accepts any `ReferenceKind` but does **not** return a `LiteralFieldRef`

## Choosing the Right ReferenceKind

Use this decision tree:

```
Is the value a reference to stored data?
    Yes -> StorePath
    No  -> Is the value a Tera template?
               Yes -> Does it need row-level data?
                          Yes -> RuntimeTeraTemplate
                          No  -> StaticTeraTemplate
               No  -> Unsupported
```

## Examples

### Mixed Fields in One Object

```rust
let (pending, fields) = builder.array_of_objects("columns", true, None);

// Literal name - can be used for derived results
let (fields, name_ref) = fields.add_literal(
    "name",
    TypeDef::Scalar(ScalarType::String),
    true,
    Some("Output column name")
);

// Static template - evaluated once
let fields = fields.add_template(
    "source_column",
    TypeDef::Scalar(ScalarType::String),
    true,
    Some("Source column (supports {{ variables }})"),
    ReferenceKind::StaticTeraTemplate
);

// Runtime template - evaluated per row
let fields = fields.add_template(
    "transform",
    TypeDef::Scalar(ScalarType::String),
    false,
    Some("Row transformation: {{ item.value * 2 }}"),
    ReferenceKind::RuntimeTeraTemplate
);

pending
    .finalise_attribute(fields)
    .derived_result("columns", name_ref, None, ResultKind::Data)
    .build()
```

### Store Path with Dependency

```rust
CommandSpecBuilder::new()
    .attribute(
        AttributeSpecBuilder::new("input", TypeDef::Tabular)
            .required()
            .reference(ReferenceKind::StorePath)
            .hint("Input data from store")
            .build()
    )
    .attribute(
        AttributeSpecBuilder::new("format", TypeDef::Scalar(ScalarType::String))
            // No .reference() call means Unsupported (default)
            .default_value(ScalarValue::String("json".to_string()))
            .build()
    )
    .fixed_result("output", TypeDef::Tabular, None, ResultKind::Data)
    .build()
```

## What Happens If You Get It Wrong

### Wrong ReferenceKind for Templates

If you mark a template field as `Unsupported`, the template syntax will not be evaluated:

```yaml
# Field has ReferenceKind::Unsupported but contains template syntax
my_command:
  path: "{{ base }}/output.csv"  # Literal string, NOT evaluated!
```

The value will be the literal string `"{{ base }}/output.csv"`, not the substituted path.

### Wrong ReferenceKind for Store Paths

If you mark a store path as `Unsupported`, dependency tracking will fail:

```yaml
# Field has ReferenceKind::Unsupported but contains a store path
my_command:
  source: "other.command.data"  # Treated as literal string, not a reference
```

The engine will not recognize this as a dependency, potentially causing execution order issues.

### Using Template Fields for Derived Results

This is prevented at compile time. If you try, you will find there is no `LiteralFieldRef` to pass:

```rust
// add_template returns just ObjectFields, not (ObjectFields, LiteralFieldRef)
let fields = fields.add_template("name", ..., ReferenceKind::StaticTeraTemplate);

// No name_ref exists - compiler error if you try to use it
// builder.derived_result("attr", name_ref, ...) // Won't compile!
```
