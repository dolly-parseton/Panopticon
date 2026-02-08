# TemplateCommand

`TemplateCommand` renders Tera templates and writes the output to a file. It supports template inheritance, includes, and loading templates from files or globs.

## When to Use

Use `TemplateCommand` when you need to:

- Generate reports, configuration files, or other text output
- Use template inheritance with base templates and blocks
- Render dynamic content using data from the pipeline
- Create multiple output files from templates

## Attributes

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `templates` | Array of objects | No | Inline template definitions (can be combined with `template_glob`) |
| `template_glob` | String | No | Glob pattern to load templates from disk (e.g., `templates/**/*.tera`) |
| `render` | String | Yes | Name of the template to render (supports Tera substitution) |
| `output` | String | Yes | File path to write the rendered output (supports Tera substitution) |
| `capture` | Boolean | No | If `true`, store the rendered content in the `content` result (default: `false`) |

At least one of `templates` or `template_glob` must provide the template to render.

### Template Object Fields

Each object in the `templates` array defines one template:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | String | Yes | Name to register the template under |
| `content` | String | No | Raw template content (mutually exclusive with `file`) |
| `file` | String | No | Path to template file (mutually exclusive with `content`) |

You must specify either `content` or `file`, but not both.

## Results

### Meta Results

| Result | Type | Description |
|--------|------|-------------|
| `line_count` | Number | Number of lines in the rendered output |
| `size` | Number | Size in bytes of the rendered output |

### Data Results

| Result | Type | Description |
|--------|------|-------------|
| `content` | String | The rendered content (only populated when `capture` is `true`, otherwise empty) |

## Examples

### Simple Inline Template

```rust
use panopticon_core::prelude::*;

// Set up data for the template
pipeline
    .add_namespace(
        NamespaceBuilder::new("inputs")
            .static_ns()
            .insert("title", ScalarValue::String("Monthly Report".to_string()))
            .insert("date", ScalarValue::String("2024-01-15".to_string())),
    )
    .await?;

let attrs = ObjectBuilder::new()
    .insert(
        "templates",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "report")
                .insert("content", "# {{ inputs.title }}\n\nGenerated on: {{ inputs.date }}")
                .build_scalar(),
        ]),
    )
    .insert("render", "report")
    .insert("output", "/tmp/report.md")
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("generate"))
    .await?
    .add_command::<TemplateCommand>("report", &attrs)
    .await?;
```

### Template Inheritance with Glob Loading

Load templates from disk using a glob pattern:

```rust
// Directory structure:
// templates/
//   base.tera       - {% block content %}{% endblock %}
//   header.tera     - Navigation HTML
//   page.tera       - {% extends "base.tera" %}{% block content %}...{% endblock %}

let attrs = ObjectBuilder::new()
    .insert("template_glob", "templates/**/*.tera")
    .insert("render", "page.tera")
    .insert("output", "/tmp/page.html")
    .insert("capture", true)
    .build_hashmap();

pipeline
    .add_namespace(NamespaceBuilder::new("render"))
    .await?
    .add_command::<TemplateCommand>("page", &attrs)
    .await?;
```

### Template Inheritance with Inline Templates

Define a base template and a child template inline:

```rust
let attrs = ObjectBuilder::new()
    .insert(
        "templates",
        ScalarValue::Array(vec![
            // Base template with blocks
            ObjectBuilder::new()
                .insert("name", "base")
                .insert("content", r#"<!DOCTYPE html>
<html>
<head><title>{% block title %}Default Title{% endblock %}</title></head>
<body>
{% block content %}{% endblock %}
</body>
</html>"#)
                .build_scalar(),
            // Child template that extends base
            ObjectBuilder::new()
                .insert("name", "page")
                .insert("content", r#"{% extends "base" %}
{% block title %}{{ inputs.page_title }}{% endblock %}
{% block content %}
<h1>{{ inputs.page_title }}</h1>
<p>{{ inputs.page_content }}</p>
{% endblock %}"#)
                .build_scalar(),
        ]),
    )
    .insert("render", "page")
    .insert("output", "/tmp/page.html")
    .build_hashmap();
```

### Loading Templates from Files

Reference template files instead of inline content:

```rust
let attrs = ObjectBuilder::new()
    .insert(
        "templates",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "base")
                .insert("file", "templates/base.tera")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "page")
                .insert("file", "templates/page.tera")
                .build_scalar(),
        ]),
    )
    .insert("render", "page")
    .insert("output", "/tmp/output.html")
    .build_hashmap();
```

### Using Pipeline Data in Templates

Templates have access to all scalar values in the store:

```rust
// Load and aggregate data
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

// Use aggregation results in template
let template_attrs = ObjectBuilder::new()
    .insert(
        "templates",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "summary")
                .insert("content", r#"Product Summary
===============
Total products: {{ stats.summary.total_count }}
Total value: ${{ stats.summary.total_value }}

{% if stats.summary.total_count > 100 %}
Note: High product count!
{% endif %}"#)
                .build_scalar(),
        ]),
    )
    .insert("render", "summary")
    .insert("output", "/tmp/summary.txt")
    .insert("capture", true)
    .build_hashmap();
```

### Dynamic Output Path

Use Tera substitution for the output path:

```rust
pipeline
    .add_namespace(
        NamespaceBuilder::new("config")
            .static_ns()
            .insert("output_dir", ScalarValue::String("/var/reports".to_string()))
            .insert("report_name", ScalarValue::String("monthly".to_string())),
    )
    .await?;

let attrs = ObjectBuilder::new()
    .insert("templates", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "report")
            .insert("content", "Report content here...")
            .build_scalar(),
    ]))
    .insert("render", "report")
    .insert("output", "{{ config.output_dir }}/{{ config.report_name }}.txt")
    .build_hashmap();
```

## Accessing Results

```rust
let completed = pipeline.compile().await?.execute().await?;
let results = completed.results(ResultSettings::default()).await?;

let source = StorePath::from_segments(["render", "page"]);
let cmd_results = results.get_by_source(&source).expect("Expected results");

// Meta results
let size = cmd_results
    .meta_get(&source.with_segment("size"))
    .expect("Expected size");
let lines = cmd_results
    .meta_get(&source.with_segment("line_count"))
    .expect("Expected line_count");

println!("Rendered {} bytes, {} lines", size, lines);

// Content (only if capture=true)
if let Some(content) = cmd_results
    .data_get(&source.with_segment("content"))
    .and_then(|r| r.as_scalar())
{
    println!("Content: {}", content.1);
}
```

## Common Patterns

### Combining Glob and Inline Templates

Load base templates from disk, add custom templates inline:

```rust
let attrs = ObjectBuilder::new()
    .insert("template_glob", "templates/**/*.tera")  // Load from disk
    .insert(
        "templates",
        ScalarValue::Array(vec![
            // Add or override templates
            ObjectBuilder::new()
                .insert("name", "custom_page")
                .insert("content", r#"{% extends "base.tera" %}
{% block content %}Custom content here{% endblock %}"#)
                .build_scalar(),
        ]),
    )
    .insert("render", "custom_page")
    .insert("output", "/tmp/custom.html")
    .build_hashmap();
```

### Iterating Over Data with Tera

Use Tera's for loop to iterate over arrays:

```rust
pipeline
    .add_namespace(
        NamespaceBuilder::new("inputs")
            .static_ns()
            .insert(
                "items",
                ScalarValue::Array(vec![
                    ObjectBuilder::new()
                        .insert("name", "Item 1")
                        .insert("price", 10.0)
                        .build_scalar(),
                    ObjectBuilder::new()
                        .insert("name", "Item 2")
                        .insert("price", 20.0)
                        .build_scalar(),
                ]),
            ),
    )
    .await?;

let attrs = ObjectBuilder::new()
    .insert("templates", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "list")
            .insert("content", r#"Items:
{% for item in inputs.items %}
- {{ item.name }}: ${{ item.price }}
{% endfor %}"#)
            .build_scalar(),
    ]))
    .insert("render", "list")
    .insert("output", "/tmp/items.txt")
    .build_hashmap();
```

### Using Tera Includes

Include templates within other templates:

```rust
let attrs = ObjectBuilder::new()
    .insert(
        "templates",
        ScalarValue::Array(vec![
            ObjectBuilder::new()
                .insert("name", "header")
                .insert("content", "<header>{{ inputs.site_name }}</header>")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "footer")
                .insert("content", "<footer>Copyright 2024</footer>")
                .build_scalar(),
            ObjectBuilder::new()
                .insert("name", "page")
                .insert("content", r#"{% include "header" %}
<main>Page content</main>
{% include "footer" %}"#)
                .build_scalar(),
        ]),
    )
    .insert("render", "page")
    .insert("output", "/tmp/page.html")
    .build_hashmap();
```

### Using Condition Results in Templates

Reference [ConditionCommand](./condition-command.md) results:

```rust
// Condition first
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

// Template using condition result
let template_attrs = ObjectBuilder::new()
    .insert("templates", ScalarValue::Array(vec![
        ObjectBuilder::new()
            .insert("name", "status")
            .insert("content", r#"System Status: {{ check.health.result }}
{% if check.health.matched %}
(condition matched at branch {{ check.health.branch_index }})
{% else %}
(using default value)
{% endif %}"#)
            .build_scalar(),
    ]))
    .insert("render", "status")
    .insert("output", "/tmp/status.txt")
    .build_hashmap();
```

## Tera Features

`TemplateCommand` uses the Tera templating engine, which supports:

- Variable substitution: `{{ variable }}`
- Conditionals: `{% if condition %}...{% endif %}`
- Loops: `{% for item in items %}...{% endfor %}`
- Template inheritance: `{% extends "base" %}` and `{% block name %}...{% endblock %}`
- Includes: `{% include "partial" %}`
- Filters: `{{ value | upper }}`, `{{ value | length }}`, etc.
- Built-in functions and operators

See the [Tera documentation](https://keats.github.io/tera/docs/) for complete syntax reference.

## Error Handling

`TemplateCommand` will return an error if:

- A template file specified in `file` does not exist
- Both `content` and `file` are specified for a template (mutually exclusive)
- Neither `content` nor `file` is specified for a template
- The `template_glob` pattern is invalid or cannot be read
- The template specified in `render` does not exist
- Template syntax errors (unclosed blocks, invalid expressions)
- Referenced variables do not exist in the scalar store
- The output directory cannot be created
- The output file cannot be written

## Directory Creation

`TemplateCommand` automatically creates parent directories for the output file if they do not exist.
