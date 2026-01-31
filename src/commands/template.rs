use crate::imports::*;

static TEMPLATECOMMAND_SPEC: CommandSchema = LazyLock::new(|| {
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
            .fixed_result("content", TypeDef::Scalar(ScalarType::String), Some("The rendered content (only populated when 'capture' is true, otherwise empty)"), ResultKind::Data)
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
                    tera.add_raw_template(name, content).map_err(|e| {
                        anyhow::anyhow!("Failed to add raw template '{}': {}", name, e)
                    })?;
                }
                TemplateSource::File { name, path } => {
                    if !path.exists() {
                        tracing::warn!(path = %path.display(), "Template file does not exist");
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
    // Going to redo these.
}