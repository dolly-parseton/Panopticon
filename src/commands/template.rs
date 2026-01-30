use crate::imports::*;

static TEMPLATECOMMAND_SPEC: LazyLock<(Vec<AttributeSpec<&'static str>>, Vec<ResultSpec<&'static str>>)> =
    LazyLock::new(|| {
        let (pending, fields) = CommandSpecBuilder::new().array_of_objects(
            "templates",
            false,
            Some("Array of template definitions. Can be combined with 'template_glob'"),
        );

        let (fields, _) = fields.add_literal(
            "name",
            TypeDef::Scalar(ScalarType::String),
            true,
            Some("Name to register the template under"),
        );
        let fields = fields.add_template(
            "content",
            TypeDef::Scalar(ScalarType::String),
            false,
            Some("Raw template content (mutually exclusive with 'file')"),
            ReferenceKind::StaticTeraTemplate,
        );
        let fields = fields.add_template(
            "file",
            TypeDef::Scalar(ScalarType::String),
            false,
            Some("Path to template file (mutually exclusive with 'content'). Dependencies within external files are not validated prior to execution"),
            ReferenceKind::StaticTeraTemplate,
        );

        pending
            .finalise_attribute(fields)
            .attribute(AttributeSpec {
                name: "template_glob",
                ty: TypeDef::Scalar(ScalarType::String),
                required: false,
                hint: Some("Glob pattern to load templates from disk (e.g., 'templates/**/*.tera'). Can be combined with 'templates' (supports Tera substitution). Dependencies within external files are not validated prior to execution"),
                default_value: None,
                reference_kind: ReferenceKind::StaticTeraTemplate,
            })
            .attribute(AttributeSpec {
                name: "render",
                ty: TypeDef::Scalar(ScalarType::String),
                required: true,
                hint: Some("Name of the template to render (supports Tera substitution)"),
                default_value: None,
                reference_kind: ReferenceKind::StaticTeraTemplate,
            })
            .attribute(AttributeSpec {
                name: "output",
                ty: TypeDef::Scalar(ScalarType::String),
                required: true,
                hint: Some("File path to write the rendered output (supports Tera substitution)"),
                default_value: None,
                reference_kind: ReferenceKind::StaticTeraTemplate,
            })
            .attribute(AttributeSpec {
                name: "capture",
                ty: TypeDef::Scalar(ScalarType::Bool),
                required: false,
                hint: Some("If true, store the rendered content in the 'content' result"),
                default_value: Some(ScalarValue::Bool(false)),
                reference_kind: ReferenceKind::Unsupported,
            })
            .fixed_result("line_count", TypeDef::Scalar(ScalarType::Number), Some("Number of lines in the rendered output"), ResultKind::Meta)
            .fixed_result("size", TypeDef::Scalar(ScalarType::Number), Some("Size in bytes of the rendered output"), ResultKind::Meta)
            .fixed_result("content", TypeDef::Scalar(ScalarType::String), Some("The rendered content (only populated when 'capture' is true, otherwise empty)"), ResultKind::Meta)
            .build()
    });

#[derive(Debug)]
enum TemplateSource {
    Raw { name: String, content: String },
    File { name: String, path: PathBuf },
}

#[derive(Debug)]
pub struct TemplateCommand {
    templates: Vec<TemplateSource>,
    template_glob: Option<String>,
    render: String,
    output: String,
    capture: bool,
}

#[async_trait::async_trait]
impl Executable for TemplateCommand {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        tracing::info!(
            template_count = self.templates.len(),
            has_glob = self.template_glob.is_some(),
            render = %self.render,
            capture = self.capture,
            "Executing TemplateCommand"
        );

        // Build the Tera instance
        let mut tera = match &self.template_glob {
            Some(glob) => tera::Tera::new(glob).map_err(|e| {
                anyhow::anyhow!("Failed to load templates from glob '{}': {}", glob, e)
            })?,
            None => tera::Tera::default(),
        };

        // Add individual templates
        for template_source in &self.templates {
            match template_source {
                TemplateSource::Raw { name, content } => {
                    tracing::debug!(name = %name, "Adding raw template");
                    tera.add_raw_template(name, content).map_err(|e| {
                        anyhow::anyhow!("Failed to add raw template '{}': {}", name, e)
                    })?;
                }
                TemplateSource::File { name, path } => {
                    tracing::debug!(name = %name, path = %path.display(), "Adding template from file");
                    if !path.exists() {
                        return Err(anyhow::anyhow!(
                            "Template file does not exist: {}",
                            path.display()
                        ));
                    }
                    tera.add_template_file(path, Some(name)).map_err(|e| {
                        anyhow::anyhow!(
                            "Failed to add template file '{}' as '{}': {}",
                            path.display(),
                            name,
                            e
                        )
                    })?;
                }
            }
        }

        // Render the template
        let rendered = context
            .scalar()
            .render_with_tera(&tera, &self.render)
            .await?;

        // Substitute the output path
        let output_path_str = context.substitute(&self.output).await?;
        let output_path = PathBuf::from(&output_path_str);

        // Create parent directories if needed
        if let Some(parent) = output_path.parent()
            && !parent.exists()
        {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create output directory '{}': {}",
                    parent.display(),
                    e
                )
            })?;
        }

        // Write the rendered content to the output file
        tokio::fs::write(&output_path, &rendered)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to write rendered output to '{}': {}",
                    output_path.display(),
                    e
                )
            })?;

        // Calculate metrics
        let size = rendered.len() as u64;
        let line_count = rendered.lines().count() as i64;

        tracing::info!(
            output = %output_path.display(),
            size = size,
            line_count = line_count,
            "Template rendered successfully"
        );

        // Store results
        let out = InsertBatch::new(context, output_prefix);
        out.i64("line_count", line_count).await?;
        out.u64("size", size).await?;

        if self.capture {
            out.string("content", rendered).await?;
        }

        Ok(())
    }
}

impl Descriptor for TemplateCommand {
    fn command_type() -> &'static str {
        "TemplateCommand"
    }
    fn command_attributes() -> &'static [AttributeSpec<&'static str>] {
        &TEMPLATECOMMAND_SPEC.0
    }
    fn command_results() -> &'static [ResultSpec<&'static str>] {
        &TEMPLATECOMMAND_SPEC.1
    }
}

impl FromAttributes for TemplateCommand {
    fn from_attributes(attrs: &Attributes) -> Result<Self> {
        // Parse templates array (optional)
        let mut templates = Vec::new();
        if let Some(templates_value) = attrs.get("templates") {
            let templates_array = templates_value.as_array_or_err("templates")?;

            for (i, template_value) in templates_array.iter().enumerate() {
                let template_obj = template_value.as_object_or_err(&format!("templates[{}]", i))?;

                let name = template_obj
                    .get_required_string("name")
                    .context(format!("templates[{}]", i))?;

                let content = template_obj.get_optional_string("content");
                let file = template_obj.get_optional_string("file");

                match (content, file) {
                    (Some(content), None) => {
                        templates.push(TemplateSource::Raw { name, content });
                    }
                    (None, Some(file)) => {
                        templates.push(TemplateSource::File {
                            name,
                            path: PathBuf::from(file),
                        });
                    }
                    (Some(_), Some(_)) => {
                        return Err(anyhow::anyhow!(
                            "templates[{}]: 'content' and 'file' are mutually exclusive",
                            i
                        ));
                    }
                    (None, None) => {
                        return Err(anyhow::anyhow!(
                            "templates[{}]: must specify either 'content' or 'file'",
                            i
                        ));
                    }
                }
            }
        }

        let template_glob = attrs.get_optional_string("template_glob");
        let render = attrs.get_required_string("render")?;
        let output = attrs.get_required_string("output")?;
        let capture = attrs.get_optional_bool("capture").unwrap_or(false);

        Ok(TemplateCommand {
            templates,
            template_glob,
            render,
            output,
            capture,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init_tracing;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
    }

    /// Helper to get results from the standard command output path (exec.render.*)
    async fn get_results(
        context: &ExecutionContext,
        namespace: &str,
        command: &str,
    ) -> (i64, u64, Option<String>) {
        let prefix = StorePath::from_segments([namespace, command]);
        let line_count = context
            .scalar()
            .get(&prefix.with_segment("line_count"))
            .await
            .unwrap()
            .unwrap()
            .as_i64()
            .unwrap();
        let size = context
            .scalar()
            .get(&prefix.with_segment("size"))
            .await
            .unwrap()
            .unwrap()
            .as_u64()
            .unwrap();
        let content = context
            .scalar()
            .get(&prefix.with_segment("content"))
            .await
            .unwrap()
            .and_then(|v| v.as_str().map(|s| s.to_string()));
        (line_count, size, content)
    }

    /// Helper to build template command attributes
    fn template_attrs(
        template_name: &str,
        template_content: &str,
        render: &str,
        output: &str,
        capture: bool,
    ) -> Attributes {
        let template = ObjectBuilder::new()
            .insert("name", template_name)
            .insert("content", template_content)
            .build_scalar();

        ObjectBuilder::new()
            .insert("templates", ScalarValue::Array(vec![template]))
            .insert("render", render)
            .insert("output", output)
            .insert("capture", capture)
            .build_hashmap()
    }

    #[tokio::test]
    async fn test_raw_template_rendering() {
        init_tracing();
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("output.txt");

        let mut pipeline = Pipeline::new();

        // Add static namespace with inputs
        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("name", ScalarValue::String("World".to_string())),
            )
            .unwrap();

        // Add execution namespace

        // Add the template command - note: template references {{ inputs.name }}
        let attrs = template_attrs(
            "greeting",
            "Hello, {{ inputs.name }}!",
            "greeting",
            &output_path.to_string_lossy(),
            false,
        );

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<TemplateCommand>("render", &attrs)
            .unwrap();

        let context = pipeline.execute().await.unwrap();

        let (line_count, size, content) = get_results(&context, "exec", "render").await;
        assert_eq!(line_count, 1);
        assert_eq!(size, 13); // "Hello, World!"
        assert!(content.is_none());

        let file_content = tokio::fs::read_to_string(&output_path).await.unwrap();
        assert_eq!(file_content, "Hello, World!");
    }

    #[tokio::test]
    async fn test_capture_enabled() {
        init_tracing();
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("output.txt");

        let mut pipeline = Pipeline::new();

        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("value", ScalarValue::Number(42.into())),
            )
            .unwrap();

        let attrs = template_attrs(
            "test",
            "The answer is {{ inputs.value }}",
            "test",
            &output_path.to_string_lossy(),
            true,
        );
        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<TemplateCommand>("render", &attrs)
            .unwrap();

        let context = pipeline.execute().await.unwrap();

        let (_, _, content) = get_results(&context, "exec", "render").await;
        assert_eq!(content, Some("The answer is 42".to_string()));
    }

    #[tokio::test]
    async fn test_multiline_template() {
        init_tracing();
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("output.txt");

        let mut pipeline = Pipeline::new();

        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("title", ScalarValue::String("Report".to_string()))
                    .insert(
                        "items",
                        ScalarValue::Array(vec![
                            ScalarValue::String("Item 1".to_string()),
                            ScalarValue::String("Item 2".to_string()),
                            ScalarValue::String("Item 3".to_string()),
                        ]),
                    ),
            )
            .unwrap();

        let template_content = r#"# {{ inputs.title }}

{% for item in inputs.items %}
- {{ item }}
{% endfor %}"#;

        let attrs = template_attrs(
            "report",
            template_content,
            "report",
            &output_path.to_string_lossy(),
            true,
        );

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<TemplateCommand>("render", &attrs)
            .unwrap();

        let context = pipeline.execute().await.unwrap();

        let (line_count, _, content) = get_results(&context, "exec", "render").await;
        assert!(line_count >= 5);
        let content = content.unwrap();
        assert!(content.contains("# Report"));
        assert!(content.contains("- Item 1"));
        assert!(content.contains("- Item 2"));
        assert!(content.contains("- Item 3"));
    }

    #[tokio::test]
    async fn test_output_path_substitution() {
        init_tracing();
        let temp_dir = tempfile::tempdir().unwrap();

        let mut pipeline = Pipeline::new();

        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("filename", ScalarValue::String("dynamic".to_string())),
            )
            .unwrap();

        // Output path uses template substitution
        let output_template = format!(
            "{}/{{{{ inputs.filename }}}}.txt",
            temp_dir.path().display()
        );

        let attrs = template_attrs("test", "content", "test", &output_template, false);

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<TemplateCommand>("render", &attrs)
            .unwrap();

        pipeline.execute().await.unwrap();

        let expected_path = temp_dir.path().join("dynamic.txt");
        assert!(expected_path.exists());
    }

    #[tokio::test]
    async fn test_creates_parent_directories() {
        init_tracing();
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("nested/deep/output.txt");

        let mut pipeline = Pipeline::new();

        let attrs = template_attrs(
            "test",
            "hello",
            "test",
            &output_path.to_string_lossy(),
            false,
        );

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<TemplateCommand>("render", &attrs)
            .unwrap();

        pipeline.execute().await.unwrap();
        assert!(output_path.exists());
    }

    #[tokio::test]
    async fn test_from_attributes_raw_template() {
        init_tracing();

        let template = ObjectBuilder::new()
            .insert("name", "test")
            .insert("content", "Hello {{ name }}")
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("templates", ScalarValue::Array(vec![template]))
            .insert("render", "test")
            .insert("output", "/tmp/out.txt")
            .build_hashmap();

        let cmd = TemplateCommand::from_attributes(&attrs).unwrap();

        assert_eq!(cmd.templates.len(), 1);
        assert_eq!(cmd.render, "test");
        assert_eq!(cmd.output, "/tmp/out.txt");
        assert!(!cmd.capture);
    }

    #[tokio::test]
    async fn test_from_attributes_file_template() {
        init_tracing();

        let template = ObjectBuilder::new()
            .insert("name", "test")
            .insert("file", "/path/to/template.tera")
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("templates", ScalarValue::Array(vec![template]))
            .insert("render", "test")
            .insert("output", "/tmp/out.txt")
            .insert("capture", true)
            .build_hashmap();

        let cmd = TemplateCommand::from_attributes(&attrs).unwrap();

        assert_eq!(cmd.templates.len(), 1);
        assert!(cmd.capture);
    }

    #[tokio::test]
    async fn test_from_attributes_rejects_both_content_and_file() {
        init_tracing();

        let template = ObjectBuilder::new()
            .insert("name", "test")
            .insert("content", "Hello")
            .insert("file", "/path/to/template.tera")
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("templates", ScalarValue::Array(vec![template]))
            .insert("render", "test")
            .insert("output", "/tmp/out.txt")
            .build_hashmap();

        let result = TemplateCommand::from_attributes(&attrs);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("mutually exclusive"));
    }

    #[tokio::test]
    async fn test_from_attributes_rejects_neither_content_nor_file() {
        init_tracing();

        // Missing both content and file
        let template = ObjectBuilder::new().insert("name", "test").build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("templates", ScalarValue::Array(vec![template]))
            .insert("render", "test")
            .insert("output", "/tmp/out.txt")
            .build_hashmap();

        let result = TemplateCommand::from_attributes(&attrs);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("must specify either"));
    }

    #[tokio::test]
    async fn test_template_not_found_error() {
        init_tracing();
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("output.txt");

        let context = ExecutionContext::new();
        let prefix = StorePath::from_segments(["ns", "cmd"]);

        let cmd = TemplateCommand {
            templates: vec![TemplateSource::Raw {
                name: "exists".to_string(),
                content: "hello".to_string(),
            }],
            template_glob: None,
            render: "nonexistent".to_string(),
            output: output_path.to_string_lossy().to_string(),
            capture: false,
        };

        let result = cmd.execute(&context, &prefix).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("nonexistent"));
    }

    #[tokio::test]
    async fn test_factory_builds_and_executes() {
        init_tracing();

        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("output.txt");

        // Verify that the factory can build an executable from attributes
        let template = ObjectBuilder::new()
            .insert("name", "greeting")
            .insert("content", "Hello, {{ inputs.name }}!")
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("templates", ScalarValue::Array(vec![template]))
            .insert("render", "greeting")
            .insert("output", output_path.to_string_lossy().to_string())
            .build_hashmap();

        // Test that factory() returns a valid builder
        let factory = TemplateCommand::factory();
        let _executable = factory(&attrs).expect("Factory should succeed");

        // Now use Pipeline::execute() pattern to actually run the template
        let mut pipeline = Pipeline::new();

        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("name", ScalarValue::String("Factory".to_string())),
            )
            .unwrap();

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<TemplateCommand>("factory_test", &attrs)
            .unwrap();

        pipeline.execute().await.unwrap();

        let file_content: String = tokio::fs::read_to_string(&output_path).await.unwrap();
        assert_eq!(file_content, "Hello, Factory!");
    }

    #[tokio::test]
    async fn test_template_glob_with_inheritance() {
        init_tracing();
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("output.html");

        let mut pipeline = Pipeline::new();

        // Add static namespace with inputs for templates
        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert("site_name", ScalarValue::String("My Site".to_string()))
                    .insert("page_title", ScalarValue::String("Welcome".to_string()))
                    .insert(
                        "page_content",
                        ScalarValue::String("This is the main content.".to_string()),
                    )
                    .insert(
                        "nav_items",
                        ScalarValue::Array(vec![
                            ObjectBuilder::new()
                                .insert("url", "/")
                                .insert("label", "Home")
                                .build_scalar(),
                            ObjectBuilder::new()
                                .insert("url", "/about")
                                .insert("label", "About")
                                .build_scalar(),
                        ]),
                    ),
            )
            .unwrap();

        // Use template_glob to load all templates from fixtures/tera/
        let glob_pattern = fixtures_dir()
            .join("tera")
            .join("**")
            .join("*")
            .to_string_lossy()
            .to_string();

        let attrs = ObjectBuilder::new()
            .insert("template_glob", glob_pattern)
            .insert("render", "page.tera")
            .insert("output", output_path.to_string_lossy().to_string())
            .insert("capture", true)
            .build_hashmap();

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<TemplateCommand>("glob_test", &attrs)
            .unwrap();

        let context = pipeline.execute().await.unwrap();

        let (line_count, size, content) = get_results(&context, "exec", "glob_test").await;
        let content = content.expect("Content should be captured");

        // Verify the rendered output contains elements from all templates
        assert!(
            content.contains("<!DOCTYPE html>"),
            "Should have base template doctype"
        );
        assert!(
            content.contains("<title>Welcome - My Site</title>"),
            "Should have page title from page.tera"
        );
        assert!(
            content.contains("<h1>My Site</h1>"),
            "Should have site name from header.tera"
        );
        assert!(
            content.contains("<a href=\"/\">Home</a>"),
            "Should have nav items from header.tera"
        );
        assert!(
            content.contains("<a href=\"/about\">About</a>"),
            "Should have nav items from header.tera"
        );
        assert!(
            content.contains("<h2>Welcome</h2>"),
            "Should have page title in content"
        );
        assert!(
            content.contains("<p>This is the main content.</p>"),
            "Should have page content"
        );
        assert!(
            content.contains("Generated by Panopticon"),
            "Should have footer from base.tera"
        );

        assert!(line_count > 10, "Should have multiple lines");
        assert!(size > 0, "Should have non-zero size");
    }

    #[tokio::test]
    async fn test_template_glob_combined_with_templates() {
        init_tracing();
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("output.html");

        let mut pipeline = Pipeline::new();

        // Add static namespace with inputs
        pipeline
            .add_namespace(
                NamespaceBuilder::new("inputs")
                    .static_ns()
                    .insert(
                        "site_name",
                        ScalarValue::String("Combined Test".to_string()),
                    )
                    .insert("page_title", ScalarValue::String("Custom Page".to_string()))
                    .insert(
                        "custom_footer",
                        ScalarValue::String("Custom footer text".to_string()),
                    )
                    .insert("nav_items", ScalarValue::Array(vec![])),
            )
            .unwrap();

        let glob_pattern = fixtures_dir()
            .join("tera")
            .join("**")
            .join("*")
            .to_string_lossy()
            .to_string();

        // Add a custom template that extends the base but overrides the footer
        let custom_template = ObjectBuilder::new()
            .insert("name", "custom_page.tera")
            .insert(
                "content",
                r#"{% extends "base.tera" %}
{% block title %}{{ inputs.page_title }}{% endblock %}

{% block header %}
{% include "header.tera" %}
{% endblock %}

{% block content %}
<p>Custom content here</p>
<footer>{{ inputs.custom_footer }}</footer>
{% endblock %}"#,
            )
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("templates", ScalarValue::Array(vec![custom_template]))
            .insert("template_glob", glob_pattern)
            .insert("render", "custom_page.tera")
            .insert("output", output_path.to_string_lossy().to_string())
            .insert("capture", true)
            .build_hashmap();

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<TemplateCommand>("combined_test", &attrs)
            .unwrap();

        let context = pipeline.execute().await.unwrap();

        let (_, _, content) = get_results(&context, "exec", "combined_test").await;
        let content = content.expect("Content should be captured");

        // Verify the custom template works with glob-loaded base templates
        assert!(
            content.contains("<title>Custom Page</title>"),
            "Should have custom title"
        );
        assert!(
            content.contains("<h1>Combined Test</h1>"),
            "Should have site name from header.tera"
        );
        assert!(
            content.contains("Custom footer text"),
            "Should have custom footer"
        );
        assert!(
            content.contains("Generated by Panopticon"),
            "Should still have base footer"
        );
    }
}
