# ConditionCommand

`ConditionCommand` evaluates Tera expressions to select between multiple branches. It provides if/then branching logic for pipelines, producing a result based on the first matching condition.

## When to Use

Use `ConditionCommand` when you need to:

- Choose between different values based on runtime conditions
- Implement feature flags or configuration-based branching
- Generate different outputs based on data characteristics
- Create conditional messages or labels

## Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `branches` | Array of objects | Yes | Array of condition branches evaluated in order |
| `default` | String | No | Default value if no branch matches (supports Tera substitution) |

### Branch Object Fields

Each object in the `branches` array defines one condition:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | String | Yes | Unique identifier for this branch |
| `if` | String | Yes | Tera expression to evaluate as the condition |
| `then` | String | Yes | Value if condition is true (supports Tera substitution) |

## Results

### Data Results (Fixed)

| Result | Type | Description |
|--------|------|-------------|
| `result` | String | The value from the matched branch or default |
| `matched` | Boolean | Whether a branch condition matched (`false` if default was used) |
| `branch_index` | Number | Index of the matched branch (0-based), or -1 if default was used |

### Data Results (Per Branch)

For each branch in the `branches` array, an object is stored:

| Result | Type | Description |
|--------|------|-------------|
| `{name}` | Object | Contains `matched` (bool) and `value` (string) for this branch |

## Condition Evaluation

The `if` expression is wrapped in `{{ }}` and evaluated as a Tera expression. The result is considered truthy if it is:

- A non-empty string (except `"false"`, `"0"`, `"null"`, `"undefined"`)
- A non-zero number
- Boolean `true`

Branches are evaluated in order. The first truthy branch wins, and subsequent branches are not evaluated.

## Examples

### Basic Conditional

```rust
use panopticon_core::prelude::*;

// Set up inputs
pipeline
    .add_namespace(
        NamespaceBuilder::new("inputs")
            .static_ns()
            .insert("user_role", ScalarValue::String("admin".to_string())),
    )
    .await?;

let attrs = ObjectBuilder::new()
    .insert(
        "branches",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "admin_greeting")
                .insert("if", "inputs.user_role == 'admin'")
                .insert("then", "Welcome, Administrator!")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "user_greeting")
                .insert("if", "inputs.user_role == 'user'")
                .insert("then", "Welcome, User!")
                .build_scalar(),
        ]),
    )
    .insert("default", "Welcome, Guest!")
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("greeting"))
    .await?
    .add_command::<ConditionCommand>("message", &attrs)
    .await?;

// Result: "Welcome, Administrator!"
```

### Feature Flag Pattern

```rust
// Static namespace with feature flags
pipeline
    .add_namespace(
        NamespaceBuilder::new("features")
            .static_ns()
            .insert("new_dashboard", ScalarValue::Bool(true))
            .insert("beta_api", ScalarValue::Bool(false)),
    )
    .await?;

let attrs = ObjectBuilder::new()
    .insert(
        "branches",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "new_dash")
                .insert("if", "features.new_dashboard")
                .insert("then", "/v2/dashboard")
                .build_scalar(),
        ]),
    )
    .insert("default", "/v1/dashboard")
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("routing"))
    .await?
    .add_command::<ConditionCommand>("dashboard_path", &attrs)
    .await?;
```

### Data-Driven Conditions

Use values from earlier pipeline stages (like aggregations):

```rust
// Aggregate order data
let agg_attrs = ObjectBuilder::new()
    .insert("source", "data.load.orders.data")
    .insert("aggregations", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "order_count")
            .insert("op", "count")
            .build_scalar(),
        ObjectBuilder::new()
            .insert("name", "total_revenue")
            .insert("column", "total")
            .insert("op", "sum")
            .build_scalar(),
    ]))
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("metrics"))
    .await?
    .add_command::<AggregateCommand>("orders", &agg_attrs)
    .await?;

// Branch based on metrics
let condition_attrs = ObjectBuilder::new()
    .insert(
        "branches",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "high_volume")
                .insert("if", "metrics.orders.order_count > 1000")
                .insert("then", "HIGH_VOLUME")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "high_revenue")
                .insert("if", "metrics.orders.total_revenue > 50000")
                .insert("then", "HIGH_REVENUE")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "normal")
                .insert("if", "true")  // Always matches as fallback
                .insert("then", "NORMAL")
                .build_scalar(),
        ]),
    )
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("classification"))
    .await?
    .add_command::<ConditionCommand>("tier", &condition_attrs)
    .await?;
```

### Using Tera Filters and Functions

The `if` expression supports full Tera syntax:

```rust
let attrs = ObjectBuilder::new()
    .insert(
        "branches",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "long_name")
                .insert("if", "inputs.name | length > 10")
                .insert("then", "Name is long")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "uppercase")
                .insert("if", "inputs.name == inputs.name | upper")
                .insert("then", "Name is all uppercase")
                .build_scalar(),
        ]),
    )
    .insert("default", "Name is normal")
    .build_hashmap();
```

## Accessing Results

```rust
let completed = pipeline.compile().await?.execute().await?;
let results = completed.results(ResultSettings::default()).await?;

let source = StorePath::from_segments(["greeting", "message"]);
let cmd_results = results.get_by_source(&source).expect("Expected results");

// Main result
let result = cmd_results
    .data_get(&source.with_segment("result"))
    .and_then(|r| r.as_scalar())
    .expect("Expected result");
println!("Result: {}", result.1);

// Did any branch match?
let matched = cmd_results
    .data_get(&source.with_segment("matched"))
    .and_then(|r| r.as_scalar())
    .expect("Expected matched");
println!("Matched: {}", matched.1);

// Which branch matched?
let index = cmd_results
    .data_get(&source.with_segment("branch_index"))
    .and_then(|r| r.as_scalar())
    .expect("Expected branch_index");
println!("Branch index: {}", index.1);  // 0, 1, 2... or -1 for default
```

## Common Patterns

### Conditional with `when` Attribute

Combine `ConditionCommand` with the `when` attribute to skip the entire command:

```rust
let attrs = ObjectBuilder::new()
    .insert("when", "inputs.feature_enabled")  // Skip if false
    .insert(
        "branches",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "greeting")
                .insert("if", "true")
                .insert("then", "Hello, {{ inputs.user_name }}! Feature is active.")
                .build_scalar(),
        ]),
    )
    .insert("default", "Fallback message")
    .build_hashmap();
```

When `when` is false:
- The command status is `"skipped"`
- No data results are produced
- `result`, `matched`, and `branch_index` are absent

### Multiple Independent Conditions

Evaluate multiple conditions that don't depend on each other:

```rust
let attrs = ObjectBuilder::new()
    .insert(
        "branches",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "is_admin")
                .insert("if", "inputs.role == 'admin'")
                .insert("then", "true")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "is_premium")
                .insert("if", "inputs.subscription == 'premium'")
                .insert("then", "true")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "has_access")
                .insert("if", "inputs.access_granted")
                .insert("then", "true")
                .build_scalar(),
        ]),
    )
    .build_hashmap();

// Access individual branch results:
// condition.check.is_admin.matched
// condition.check.is_premium.matched
// condition.check.has_access.matched
```

### Cascading If/Else

Use `true` as the final condition for an else clause:

```rust
let attrs = ObjectBuilder::new()
    .insert(
        "branches",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "premium")
                .insert("if", "metrics.score > 90")
                .insert("then", "PREMIUM")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "standard")
                .insert("if", "metrics.score > 50")
                .insert("then", "STANDARD")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "basic")
                .insert("if", "true")  // Else clause
                .insert("then", "BASIC")
                .build_scalar(),
        ]),
    )
    .build_hashmap();
```

## Using Results in Templates

Condition results are stored in the scalar store and can be used in templates:

```rust
// Condition command
let condition_attrs = ObjectBuilder::new()
    .insert("branches", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "status")
            .insert("if", "metrics.health > 80")
            .insert("then", "healthy")
            .build_scalar(),
    ]))
    .insert("default", "degraded")
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("check"))
    .await?
    .add_command::<ConditionCommand>("health", &condition_attrs)
    .await?;

// Template using the result
let template_attrs = ObjectBuilder::new()
    .insert("templates", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "report")
            .insert("content", "System status: {{ check.health.result }}")
            .build_scalar(),
    ]))
    .insert("render", "report")
    .insert("output", "/tmp/status.txt")
    .build_hashmap();
```

## Error Handling

`ConditionCommand` will return an error if:

- A branch is missing required fields (`name`, `if`, or `then`)
- A Tera expression in `if` or `then` cannot be evaluated
- Referenced variables in expressions do not exist in the scalar store
