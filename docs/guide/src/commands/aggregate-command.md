# AggregateCommand

`AggregateCommand` computes scalar statistics from tabular data. It supports a variety of aggregation operations including count, sum, mean, min, max, median, and more.

## When to Use

Use `AggregateCommand` when you need to:

- Compute summary statistics from a DataFrame
- Extract scalar values (counts, sums, averages) for use in templates or conditions
- Calculate multiple aggregations in a single command
- Get values like row count, unique counts, or null counts

## Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `source` | String | Yes | Store path to tabular data (e.g., `data.load.products.data`) |
| `aggregations` | Array of objects | Yes | Array of aggregation specifications |

### Aggregation Object Fields

Each object in the `aggregations` array specifies one aggregation:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | String | Yes | Output scalar name for this aggregation |
| `column` | String | No | Column to aggregate (not required for `count`) |
| `op` | String | Yes | Aggregation operation to perform |

### Supported Operations

| Operation | Aliases | Column Required | Description |
|-----------|---------|-----------------|-------------|
| `sum` | - | Yes | Sum of values in the column |
| `mean` | `avg`, `average` | Yes | Arithmetic mean of values |
| `min` | - | Yes | Minimum value |
| `max` | - | Yes | Maximum value |
| `count` | `len` | No | Number of rows in the DataFrame |
| `first` | - | Yes | First value in the column |
| `last` | - | Yes | Last value in the column |
| `std` | `stddev` | Yes | Standard deviation |
| `median` | - | Yes | Median value |
| `n_unique` | `nunique`, `distinct` | Yes | Count of unique values |
| `null_count` | `nulls` | Yes | Count of null values in the column |

## Results

### Data Results (Per Aggregation)

For each aggregation in the `aggregations` array, a scalar result is produced:

| Result | Type | Description |
|--------|------|-------------|
| `{name}` | Scalar (Number) | The computed aggregation value |

The result path is `{output_prefix}.{name}`, where `{name}` is the `name` field from the aggregation object.

## Examples

### Basic Aggregations

```rust
use panopticon_core::prelude::*;

let attrs = ObjectBuilder::new()
    .insert("source", "data.load.products.data")
    .insert(
        "aggregations",
        ScalarValue::Array(vec![
            // Count doesn't need a column
            ObjectBuilder::new()
                .insert("name", "row_count")
                .insert("op", "count")
                .build_scalar(),
            // Sum requires a column
            ObjectBuilder::new()
                .insert("name", "total_price")
                .insert("column", "price")
                .insert("op", "sum")
                .build_scalar(),
            // Mean/average
            ObjectBuilder::new()
                .insert("name", "avg_price")
                .insert("column", "price")
                .insert("op", "mean")
                .build_scalar(),
        ]),
    )
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("stats"))
    .await?
    .add_command::<AggregateCommand>("summary", &attrs)
    .await?;

// Results available at:
// - stats.summary.row_count
// - stats.summary.total_price
// - stats.summary.avg_price
```

### Full Statistical Summary

Compute comprehensive statistics for a dataset:

```rust
let attrs = ObjectBuilder::new()
    .insert("source", "data.load.products.data")
    .insert(
        "aggregations",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "row_count")
                .insert("op", "count")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "total_price")
                .insert("column", "price")
                .insert("op", "sum")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "avg_price")
                .insert("column", "price")
                .insert("op", "mean")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "max_quantity")
                .insert("column", "quantity")
                .insert("op", "max")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "min_quantity")
                .insert("column", "quantity")
                .insert("op", "min")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "median_price")
                .insert("column", "price")
                .insert("op", "median")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "price_stddev")
                .insert("column", "price")
                .insert("op", "std")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "unique_categories")
                .insert("column", "category")
                .insert("op", "n_unique")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "missing_descriptions")
                .insert("column", "description")
                .insert("op", "null_count")
                .build_scalar(),
        ]),
    )
    .build_hashmap();
```

### First and Last Values

Extract the first or last value from a column (useful for time-series data):

```rust
let attrs = ObjectBuilder::new()
    .insert("source", "data.load.events.data")
    .insert(
        "aggregations",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "first_event")
                .insert("column", "event_type")
                .insert("op", "first")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "last_event")
                .insert("column", "event_type")
                .insert("op", "last")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "earliest_timestamp")
                .insert("column", "timestamp")
                .insert("op", "first")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "latest_timestamp")
                .insert("column", "timestamp")
                .insert("op", "last")
                .build_scalar(),
        ]),
    )
    .build_hashmap();
```

## Accessing Results

```rust
let completed = pipeline.compile().await?.execute().await?;
let results = completed.results(ResultSettings::default()).await?;

let source = StorePath::from_segments(["stats", "products"]);
let cmd_results = results.get_by_source(&source).expect("Expected results");

// Access aggregation results
let row_count = cmd_results
    .data_get(&source.with_segment("row_count"))
    .and_then(|r| r.as_scalar())
    .expect("Expected row_count");

let avg_price = cmd_results
    .data_get(&source.with_segment("avg_price"))
    .and_then(|r| r.as_scalar())
    .expect("Expected avg_price");

println!("Products: {} rows, average price: {}", row_count.1, avg_price.1);
```

## Common Patterns

### Using Aggregates in Templates

Aggregation results are stored in the scalar store and can be referenced in Tera templates:

```rust
// First: aggregate data
let agg_attrs = ObjectBuilder::new()
    .insert("source", "data.load.products.data")
    .insert("aggregations", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "total_count")
            .insert("op", "count")
            .build_scalar(),
        ObjectBuilder::new()
            .insert("name", "total_value")
            .insert("column", "price")
            .insert("op", "sum")
            .build_scalar(),
    ]))
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("stats"))
    .await?
    .add_command::<AggregateCommand>("summary", &agg_attrs)
    .await?;

// Then: use in a template
let template_attrs = ObjectBuilder::new()
    .insert("templates", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "report")
            .insert("content", "Total products: {{ stats.summary.total_count }}\nTotal value: ${{ stats.summary.total_value }}")
            .build_scalar(),
    ]))
    .insert("render", "report")
    .insert("output", "/tmp/report.txt")
    .build_hashmap();
```

### Using Aggregates in Conditions

Branch logic based on aggregation results:

```rust
// Aggregate first
let agg_attrs = ObjectBuilder::new()
    .insert("source", "data.load.orders.data")
    .insert("aggregations", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "order_count")
            .insert("op", "count")
            .build_scalar(),
    ]))
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("metrics"))
    .await?
    .add_command::<AggregateCommand>("orders", &agg_attrs)
    .await?;

// Condition based on aggregate
let condition_attrs = ObjectBuilder::new()
    .insert("branches", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "high_volume")
            .insert("if", "metrics.orders.order_count > 1000")
            .insert("then", "High order volume detected")
            .build_scalar(),
        ObjectBuilder::new()
            .insert("name", "normal")
            .insert("if", "true")
            .insert("then", "Normal order volume")
            .build_scalar(),
    ]))
    .build_hashmap();
```

### Aggregating SQL Query Results

Chain with [SqlCommand](./sql-command.md) to aggregate filtered data:

```rust
// Query
let query_attrs = ObjectBuilder::new()
    .insert("tables", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "orders")
            .insert("source", "data.load.orders.data")
            .build_scalar(),
    ]))
    .insert("query", "SELECT * FROM orders WHERE status = 'completed'")
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("query"))
    .await?
    .add_command::<SqlCommand>("completed", &query_attrs)
    .await?;

// Aggregate the filtered results
let agg_attrs = ObjectBuilder::new()
    .insert("source", "query.completed.data")  // Reference SQL output
    .insert("aggregations", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "completed_count")
            .insert("op", "count")
            .build_scalar(),
        ObjectBuilder::new()
            .insert("name", "total_revenue")
            .insert("column", "total")
            .insert("op", "sum")
            .build_scalar(),
    ]))
    .build_hashmap();
```

## Error Handling

`AggregateCommand` will return an error if:

- The source store path does not exist
- A specified column does not exist in the DataFrame
- An operation that requires a column (anything except `count`) is missing the `column` field
- The operation name is not recognized

## Type Handling

- Numeric columns return appropriate numeric types (integer or float)
- `first` and `last` operations work on both numeric and string columns
- Non-finite values (NaN, infinity) are converted to null
- Integer results are preserved when possible (e.g., count, sum of integers)
