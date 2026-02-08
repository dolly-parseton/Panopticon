# SqlCommand

`SqlCommand` executes SQL queries against tabular data stored in the pipeline. It uses Polars' SQL context to provide full SQL query capabilities on DataFrames.

## When to Use

Use `SqlCommand` when you need to:

- Filter rows based on conditions
- Select specific columns
- Join multiple tables together
- Group and aggregate data
- Transform data using SQL expressions
- Order and limit results

## Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `tables` | Array of objects | Yes | Table mappings from store paths to SQL table names |
| `query` | String | Yes | SQL query to execute (supports Tera substitution) |

### Table Object Fields

Each object in the `tables` array maps a store path to a table name for use in SQL:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | String | Yes | Table name to use in the SQL query |
| `source` | String | Yes | Store path to tabular data (e.g., `data.load.users.data`) |

## Results

### Data Results

| Result | Type | Description |
|--------|------|-------------|
| `data` | Tabular (DataFrame) | The query result |

### Meta Results

| Result | Type | Description |
|--------|------|-------------|
| `rows` | Number | Number of rows in the result |
| `columns` | Array | Column names in the result |

## Examples

### Basic Query

```rust
use panopticon_core::prelude::*;

// Assume data was loaded with FileCommand at data.load.users.data
let attrs = ObjectBuilder::new()
    .insert(
        "tables",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "users")
                .insert("source", "data.load.users.data")
                .build_scalar(),
        ]),
    )
    .insert("query", "SELECT * FROM users WHERE active = true")
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("query"))
    .await?
    .add_command::<SqlCommand>("active_users", &attrs)
    .await?;

// Result available at: query.active_users.data
```

### Joining Multiple Tables

```rust
let attrs = ObjectBuilder::new()
    .insert(
        "tables",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "users")
                .insert("source", "data.load.users.data")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "orders")
                .insert("source", "data.load.orders.data")
                .build_scalar(),
        ]),
    )
    .insert(
        "query",
        "SELECT u.name, u.email, o.order_id, o.total \
         FROM users u \
         INNER JOIN orders o ON u.id = o.user_id \
         ORDER BY o.total DESC",
    )
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("query"))
    .await?
    .add_command::<SqlCommand>("user_orders", &attrs)
    .await?;
```

### Cross Join

Combine every row from one table with every row from another:

```rust
let attrs = ObjectBuilder::new()
    .insert(
        "tables",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "users")
                .insert("source", "data.load.users.data")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "events")
                .insert("source", "data.load.events.data")
                .build_scalar(),
        ]),
    )
    .insert(
        "query",
        "SELECT u.name, u.email, e.type AS event_type, e.timestamp \
         FROM users u CROSS JOIN events e \
         ORDER BY u.name, e.timestamp",
    )
    .build_hashmap();
```

### Dynamic Query with Tera Substitution

Use Tera expressions to parameterize queries:

```rust
// Static namespace with filter values
pipeline
    .add_namespace(
        NamespaceBuilder::new("inputs")
            .static_ns()
            .insert("status", ScalarValue::String("active".to_string()))
            .insert("min_age", ScalarValue::from(18)),
    )
    .await?;

// Query with Tera substitution
let attrs = ObjectBuilder::new()
    .insert(
        "tables",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "users")
                .insert("source", "data.load.users.data")
                .build_scalar(),
        ]),
    )
    .insert(
        "query",
        "SELECT * FROM users WHERE status = '{{ inputs.status }}' AND age >= {{ inputs.min_age }}",
    )
    .build_hashmap();
```

### Aggregation in SQL

While [AggregateCommand](./aggregate-command.md) is available for simple aggregations, SQL can perform grouped aggregations:

```rust
let attrs = ObjectBuilder::new()
    .insert(
        "tables",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "orders")
                .insert("source", "data.load.orders.data")
                .build_scalar(),
        ]),
    )
    .insert(
        "query",
        "SELECT category, COUNT(*) as order_count, SUM(total) as revenue \
         FROM orders \
         GROUP BY category \
         ORDER BY revenue DESC",
    )
    .build_hashmap();
```

## Accessing Results

```rust
let completed = pipeline.compile().await?.execute().await?;
let results = completed.results(ResultSettings::default()).await?;

let source = StorePath::from_segments(["query", "active_users"]);
let cmd_results = results.get_by_source(&source).expect("Expected results");

// Access meta results
let row_count = cmd_results
    .meta_get(&source.with_segment("rows"))
    .expect("Expected rows");
let columns = cmd_results
    .meta_get(&source.with_segment("columns"))
    .expect("Expected columns");

println!("Query returned {} rows with columns: {}", row_count, columns);

// Access the DataFrame
let data_result = cmd_results
    .data_get(&source.with_segment("data"))
    .expect("Expected data");
```

## Common Patterns

### Chaining SQL Queries

Use the output of one SQL query as input to another:

```rust
// First query: filter data
let filter_attrs = ObjectBuilder::new()
    .insert("tables", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "orders")
            .insert("source", "data.load.orders.data")
            .build_scalar(),
    ]))
    .insert("query", "SELECT * FROM orders WHERE status = 'completed'")
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("step1"))
    .await?
    .add_command::<SqlCommand>("filtered", &filter_attrs)
    .await?;

// Second query: aggregate the filtered data
let agg_attrs = ObjectBuilder::new()
    .insert("tables", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "orders")
            .insert("source", "step1.filtered.data")  // Reference previous result
            .build_scalar(),
    ]))
    .insert("query", "SELECT category, SUM(total) as revenue FROM orders GROUP BY category")
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("step2"))
    .await?
    .add_command::<SqlCommand>("by_category", &agg_attrs)
    .await?;
```

### SqlCommand + AggregateCommand

Query with SQL, then compute scalar statistics with AggregateCommand:

```rust
// SQL query
let query_attrs = ObjectBuilder::new()
    .insert("tables", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "products")
            .insert("source", "data.load.products.data")
            .build_scalar(),
    ]))
    .insert("query", "SELECT * FROM products WHERE in_stock = true")
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("query"))
    .await?
    .add_command::<SqlCommand>("in_stock", &query_attrs)
    .await?;

// Aggregate the query result
let agg_attrs = ObjectBuilder::new()
    .insert("source", "query.in_stock.data")
    .insert("aggregations", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "avg_price")
            .insert("column", "price")
            .insert("op", "mean")
            .build_scalar(),
    ]))
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("stats"))
    .await?
    .add_command::<AggregateCommand>("summary", &agg_attrs)
    .await?;
```

## SQL Dialect

`SqlCommand` uses Polars' SQL context, which supports a subset of standard SQL:

- `SELECT`, `FROM`, `WHERE`, `ORDER BY`, `LIMIT`
- `JOIN` (INNER, LEFT, RIGHT, FULL, CROSS)
- `GROUP BY`, `HAVING`
- Common aggregate functions: `COUNT`, `SUM`, `AVG`, `MIN`, `MAX`
- String functions, date functions, and more

Refer to the [Polars SQL documentation](https://docs.pola.rs/user-guide/sql/intro/) for the complete list of supported features.

## Error Handling

`SqlCommand` will return an error if:

- A table source store path does not exist
- The SQL query syntax is invalid
- A referenced column does not exist in the table
- The query execution fails for any reason
