# Getting Started

This guide will walk us through installing Panopticon and running our first data pipeline. By the end, we will understand the basic pattern for building pipelines and be ready to explore more advanced features.

## Installation

Add `panopticon-core` to your project's `Cargo.toml`:

```toml
[dependencies]
panopticon-core = "0.2"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
anyhow = "1"
```

Panopticon uses [Tokio](https://tokio.rs/) for async runtime and [anyhow](https://docs.rs/anyhow) for error handling. These are required dependencies for running pipelines.

## Hello Pipeline

Let us build a minimal pipeline that loads a CSV file and prints some basic information about it. This demonstrates the core pattern we will use throughout Panopticon.

```rust
use panopticon_core::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Create a new pipeline
    let mut pipeline = Pipeline::new();

    // 2. Define file loading attributes
    let file_attrs = ObjectBuilder::new()
        .insert(
            "files",
            ScalarValue::Array(vec![
                ObjectBuilder::new()
                    .insert("name", "products")
                    .insert("file", "/path/to/products.csv")
                    .insert("format", "csv")
                    .build_scalar(),
            ]),
        )
        .build_hashmap();

    // 3. Add a namespace and command to the pipeline
    pipeline
        .add_namespace(NamespaceBuilder::new("data"))
        .await?
        .add_command::<FileCommand>("load", &file_attrs)
        .await?;

    // 4. Compile and execute
    let completed = pipeline.compile().await?.execute().await?;

    // 5. Access results
    let results = completed.results(ResultSettings::default()).await?;

    println!("Pipeline completed with {} command(s)", results.len());

    Ok(())
}
```

Let us break down what is happening:

1. **Pipeline::new()** creates a draft pipeline ready to accept namespaces and commands.

2. **ObjectBuilder** constructs the attribute map that configures our command. Here we define a file to load with its name, path, and format.

3. **add_namespace()** creates a logical grouping, and **add_command()** adds a command to that namespace. The command is identified by `namespace.command` (e.g., `data.load`).

4. **compile()** validates the pipeline and resolves dependencies. **execute()** runs all commands in the correct order.

5. **results()** returns a `ResultStore` containing all command outputs, which we can query by path.

## Running Examples

The Panopticon repository includes several examples demonstrating different features. To run them:

```bash
# Clone the repository
git clone https://github.com/dolly-parseton/panopticon.git
cd panopticon

# Run the multi-format loading example
cargo run --example multi_format_load

# Run the aggregation example
cargo run --example aggregate_and_export

# Run the conditional execution example
cargo run --example when_conditional
```

Each example is self-contained and includes comments explaining what it demonstrates.

### Available Examples

| Example | Description |
|---------|-------------|
| `multi_format_load` | Loading CSV, JSON, and Parquet files |
| `aggregate_and_export` | Aggregation operations and result export |
| `when_conditional` | Conditional command execution |
| `template_inheritance` | Tera template inheritance patterns |
| `iterate_object_keys` | Iterating over dynamic data |
| `pipeline_reuse` | Reusing pipeline definitions |
| `custom_command` | Building your own commands |
| `command_spec_safety` | Command specification validation |

## Project Structure

A typical Panopticon project follows this structure:

```
my-pipeline/
├── Cargo.toml
├── src/
│   └── main.rs          # Pipeline definition and execution
├── templates/           # Tera templates (if using TemplateCommand)
│   └── report.html
└── data/                # Input data files
    ├── input.csv
    └── config.json
```

For larger projects, we recommend organizing pipelines into modules:

```
my-pipeline/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── pipelines/
│   │   ├── mod.rs
│   │   ├── etl.rs       # ETL pipeline
│   │   └── reports.rs   # Reporting pipeline
│   └── commands/        # Custom commands (if extending)
│       └── mod.rs
└── ...
```

## Next Steps

Now that we have a basic pipeline running, we are ready to explore the core concepts:

- [Core Concepts](../core-concepts/index.md) - Understand the pipeline state machine, namespaces, and data stores
- [Commands Overview](../commands/index.md) - Learn about the built-in commands available
- [Working with Data](../working-with-data/index.md) - Master store paths and data access patterns

If you want to build custom commands for your specific use case, see the [Extending Panopticon](../../extending/src/introduction.md) guide.
