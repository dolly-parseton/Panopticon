use crate::imports::*;
use polars::prelude::SerReader;

static FILECOMMAND_SPEC: LazyLock<(Vec<AttributeSpec<&'static str>>, Vec<ResultSpec<&'static str>>)> =
    LazyLock::new(|| {
        let (pending, fields) = CommandSpecBuilder::new().array_of_objects(
            "files",
            true,
            Some("Array of {name, file, format} objects to read"),
        );

        let (fields, name_ref) = fields.add_literal(
            "name",
            TypeDef::Scalar(ScalarType::String),
            true,
            Some("Identifier for this file in the TabularStore"),
        );
        let fields = fields.add_template(
            "file",
            TypeDef::Scalar(ScalarType::String),
            true,
            Some("Path to the file to read (supports tera templates)"),
            ReferenceKind::StaticTeraTemplate,
        );
        let fields = fields.add_template(
            "format",
            TypeDef::Scalar(ScalarType::String),
            true,
            Some("Format of the file: csv, json, or parquet (supports tera templates)"),
            ReferenceKind::StaticTeraTemplate,
        );

        pending
            .finalise_attribute(fields)
            .fixed_result("count", TypeDef::Scalar(ScalarType::Number), Some("The number of files loaded."), ResultKind::Meta)
            .fixed_result("total_rows", TypeDef::Scalar(ScalarType::Number), Some("The total number of rows across all loaded files."), ResultKind::Meta)
            .fixed_result("total_size", TypeDef::Scalar(ScalarType::Number), Some("The total size in bytes across all loaded files."), ResultKind::Meta)
            .derived_result("files", name_ref, Some(TypeDef::Tabular), ResultKind::Data)
            .build()
    });

struct FileSpec {
    name: String,
    file: PathBuf,
    format: String,
}

pub struct FileCommand {
    files: Vec<FileSpec>,
}

#[async_trait::async_trait]
impl Executable for FileCommand {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()> {
        tracing::info!(file_count = self.files.len(), "Executing FileCommand");

        let mut total_rows: u64 = 0;
        let mut total_size: u64 = 0;

        for file_spec in self.files.iter() {
            tracing::info!(
                name = %file_spec.name,
                file = %file_spec.file.display(),
                format = %file_spec.format,
                "Loading file"
            );

            if !file_spec.file.exists() {
                return Err(anyhow::anyhow!(
                    "File does not exist: {}",
                    file_spec.file.display()
                ));
            }
            if file_spec.file.is_dir() {
                return Err(anyhow::anyhow!(
                    "Path is a directory, not a file: {}",
                    file_spec.file.display()
                ));
            }
            let metadata = tokio::fs::metadata(&file_spec.file).await?;
            let file_size = metadata.len();

            let path = file_spec.file.clone();
            let format = file_spec.format.clone();
            let df = tokio::task::spawn_blocking(move || -> Result<polars::prelude::DataFrame> {
                match format.as_str() {
                    "csv" => polars::prelude::CsvReadOptions::default()
                        .with_has_header(true)
                        .try_into_reader_with_file_path(Some(path.clone()))?
                        .finish()
                        .map_err(|e| {
                            anyhow::anyhow!("Failed to read CSV file {}: {}", path.display(), e)
                        }),
                    "json" => {
                        let file = std::fs::File::open(&path)?;
                        polars::prelude::JsonReader::new(file)
                            .finish()
                            .map_err(|e| {
                                anyhow::anyhow!(
                                    "Failed to read JSON file {}: {}",
                                    path.display(),
                                    e
                                )
                            })
                    }
                    "parquet" => {
                        let file = std::fs::File::open(&path)?;
                        polars::prelude::ParquetReader::new(file)
                            .finish()
                            .map_err(|e| {
                                anyhow::anyhow!(
                                    "Failed to read Parquet file {}: {}",
                                    path.display(),
                                    e
                                )
                            })
                    }
                    _ => Err(anyhow::anyhow!("Unsupported file format: {}", format)),
                }
            })
            .await
            .map_err(|e| anyhow::anyhow!("Task join error: {}", e))??;

            let row_count = df.height() as u64;
            let column_names: Vec<ScalarValue> = df
                .get_column_names()
                .iter()
                .map(|name| ScalarValue::String(name.to_string()))
                .collect();

            total_rows += row_count;
            total_size += file_size;

            let file_prefix = output_prefix.with_segment(&file_spec.name);
            let out = InsertBatch::new(context, &file_prefix);

            out.tabular("data", df).await?;
            out.u64("rows", row_count).await?;
            out.u64("size", file_size).await?;
            out.scalar("columns", ScalarValue::Array(column_names))
                .await?;
        }

        // Insert summary outputs
        let out = InsertBatch::new(context, output_prefix);
        out.i64("count", self.files.len() as i64).await?;
        out.u64("total_rows", total_rows).await?;
        out.u64("total_size", total_size).await?;

        Ok(())
    }
}

impl Descriptor for FileCommand {
    fn command_type() -> &'static str {
        "FileCommand"
    }
    fn command_attributes() -> &'static [AttributeSpec<&'static str>] {
        &FILECOMMAND_SPEC.0
    }
    fn command_results() -> &'static [ResultSpec<&'static str>] {
        &FILECOMMAND_SPEC.1
    }
}

impl FromAttributes for FileCommand {
    fn from_attributes(attrs: &Attributes) -> Result<Self> {
        let files_array = attrs.get_required("files")?.as_array_or_err("files")?;

        let mut files = Vec::with_capacity(files_array.len());
        for (i, file_value) in files_array.iter().enumerate() {
            let file_obj = file_value.as_object_or_err(&format!("files[{}]", i))?;

            let name = file_obj
                .get_required_string("name")
                .context(format!("files[{}]", i))?;
            let file = file_obj
                .get_required_string("file")
                .context(format!("files[{}]", i))?;
            let format = file_obj
                .get_required_string("format")
                .context(format!("files[{}]", i))?;

            files.push(FileSpec {
                name,
                file: PathBuf::from(file),
                format,
            });
        }

        Ok(FileCommand { files })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init_tracing;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
    }

    fn make_file_spec(name: &str, file: &str, format: &str) -> FileSpec {
        FileSpec {
            name: name.to_string(),
            file: PathBuf::from(file),
            format: format.to_string(),
        }
    }

    /// Helper to build file command attributes
    fn file_attrs(files: Vec<FileSpec>) -> Attributes {
        let file_values: Vec<ScalarValue> = files
            .into_iter()
            .map(|f| {
                ObjectBuilder::new()
                    .insert("name", f.name)
                    .insert("file", f.file.to_string_lossy().to_string())
                    .insert("format", f.format)
                    .build_scalar()
            })
            .collect();

        ObjectBuilder::new()
            .insert("files", ScalarValue::Array(file_values))
            .build_hashmap()
    }

    async fn get_summary(
        context: &ExecutionContext,
        namespace: &str,
        command: &str,
    ) -> (i64, u64, u64) {
        let prefix = StorePath::from_segments([namespace, command]);
        let count = context
            .scalar()
            .get(&prefix.with_segment("count"))
            .await
            .unwrap()
            .unwrap()
            .as_i64()
            .unwrap();
        let total_rows = context
            .scalar()
            .get(&prefix.with_segment("total_rows"))
            .await
            .unwrap()
            .unwrap()
            .as_u64()
            .unwrap();
        let total_size = context
            .scalar()
            .get(&prefix.with_segment("total_size"))
            .await
            .unwrap()
            .unwrap()
            .as_u64()
            .unwrap();
        (count, total_rows, total_size)
    }

    async fn get_file_outputs(
        context: &ExecutionContext,
        namespace: &str,
        command: &str,
        name: &str,
    ) -> (u64, u64, Vec<String>) {
        let prefix = StorePath::from_segments([namespace, command]);
        let file_prefix = prefix.with_segment(name);
        let rows = context
            .scalar()
            .get(&file_prefix.with_segment("rows"))
            .await
            .unwrap()
            .unwrap()
            .as_u64()
            .unwrap();
        let size = context
            .scalar()
            .get(&file_prefix.with_segment("size"))
            .await
            .unwrap()
            .unwrap()
            .as_u64()
            .unwrap();
        let columns = context
            .scalar()
            .get(&file_prefix.with_segment("columns"))
            .await
            .unwrap()
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        (rows, size, columns)
    }

    #[tokio::test]
    async fn test_load_single_csv() {
        init_tracing();
        let mut pipeline = Pipeline::new();

        let attrs = file_attrs(vec![make_file_spec(
            "users",
            fixtures_dir().join("users.csv").to_str().unwrap(),
            "csv",
        )]);

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<FileCommand>("load", &attrs)
            .unwrap();

        let context = pipeline.execute().await.unwrap();

        let (count, total_rows, total_size) = get_summary(&context, "exec", "load").await;
        assert_eq!(count, 1);
        assert_eq!(total_rows, 3);
        assert!(total_size > 0);

        let (rows, size, columns) = get_file_outputs(&context, "exec", "load", "users").await;
        assert_eq!(rows, 3);
        assert!(size > 0);
        assert_eq!(columns, vec!["id", "name", "email", "age"]);

        // Verify tabular data exists
        let prefix = StorePath::from_segments(["exec", "load"]);
        let df = context
            .tabular()
            .get(&prefix.with_segment("users").with_segment("data"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(df.height(), 3);
        assert_eq!(df.width(), 4);
    }

    #[tokio::test]
    async fn test_load_single_json() {
        init_tracing();
        let mut pipeline = Pipeline::new();

        let attrs = file_attrs(vec![make_file_spec(
            "events",
            fixtures_dir().join("events.json").to_str().unwrap(),
            "json",
        )]);

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<FileCommand>("load", &attrs)
            .unwrap();

        let context = pipeline.execute().await.unwrap();

        let (count, total_rows, _) = get_summary(&context, "exec", "load").await;
        assert_eq!(count, 1);
        assert_eq!(total_rows, 3);

        let (rows, _, columns) = get_file_outputs(&context, "exec", "load", "events").await;
        assert_eq!(rows, 3);
        assert!(columns.contains(&"event_id".to_string()));
        assert!(columns.contains(&"type".to_string()));
        assert!(columns.contains(&"timestamp".to_string()));
    }

    #[tokio::test]
    async fn test_load_single_parquet() {
        init_tracing();
        let mut pipeline = Pipeline::new();

        let attrs = file_attrs(vec![make_file_spec(
            "metrics",
            fixtures_dir().join("metrics.parquet").to_str().unwrap(),
            "parquet",
        )]);

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<FileCommand>("load", &attrs)
            .unwrap();

        let context = pipeline.execute().await.unwrap();

        let (count, total_rows, _) = get_summary(&context, "exec", "load").await;
        assert_eq!(count, 1);
        assert_eq!(total_rows, 3);

        let (rows, _, columns) = get_file_outputs(&context, "exec", "load", "metrics").await;
        assert_eq!(rows, 3);
        assert!(columns.contains(&"id".to_string()));
        assert!(columns.contains(&"category".to_string()));
        assert!(columns.contains(&"value".to_string()));
    }

    #[tokio::test]
    async fn test_load_multiple_files() {
        init_tracing();
        let mut pipeline = Pipeline::new();

        let attrs = file_attrs(vec![
            make_file_spec(
                "users",
                fixtures_dir().join("users.csv").to_str().unwrap(),
                "csv",
            ),
            make_file_spec(
                "products",
                fixtures_dir().join("products.csv").to_str().unwrap(),
                "csv",
            ),
        ]);

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<FileCommand>("load", &attrs)
            .unwrap();

        let context = pipeline.execute().await.unwrap();

        let (count, total_rows, total_size) = get_summary(&context, "exec", "load").await;
        assert_eq!(count, 2);
        assert_eq!(total_rows, 6); // 3 + 3
        assert!(total_size > 0);

        // Check users file outputs
        let (users_rows, _, users_columns) =
            get_file_outputs(&context, "exec", "load", "users").await;
        assert_eq!(users_rows, 3);
        assert_eq!(users_columns, vec!["id", "name", "email", "age"]);

        // Check products file outputs
        let (products_rows, _, products_columns) =
            get_file_outputs(&context, "exec", "load", "products").await;
        assert_eq!(products_rows, 3);
        assert_eq!(products_columns, vec!["sku", "name", "price", "quantity"]);
    }

    #[tokio::test]
    async fn test_load_mixed_formats() {
        init_tracing();
        let mut pipeline = Pipeline::new();

        let attrs = file_attrs(vec![
            make_file_spec(
                "users",
                fixtures_dir().join("users.csv").to_str().unwrap(),
                "csv",
            ),
            make_file_spec(
                "events",
                fixtures_dir().join("events.json").to_str().unwrap(),
                "json",
            ),
            make_file_spec(
                "metrics",
                fixtures_dir().join("metrics.parquet").to_str().unwrap(),
                "parquet",
            ),
        ]);

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<FileCommand>("load", &attrs)
            .unwrap();

        let context = pipeline.execute().await.unwrap();

        let (count, total_rows, _) = get_summary(&context, "exec", "load").await;
        assert_eq!(count, 3);
        assert_eq!(total_rows, 9); // 3 + 3 + 3
    }

    #[tokio::test]
    async fn test_empty_files_array() {
        init_tracing();
        let mut pipeline = Pipeline::new();

        let attrs = file_attrs(vec![]);

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<FileCommand>("load", &attrs)
            .unwrap();

        let context = pipeline.execute().await.unwrap();

        let (count, total_rows, total_size) = get_summary(&context, "exec", "load").await;
        assert_eq!(count, 0);
        assert_eq!(total_rows, 0);
        assert_eq!(total_size, 0);
    }

    #[tokio::test]
    async fn test_file_not_found_error() {
        init_tracing();
        let context = ExecutionContext::new();
        let prefix = StorePath::from_segments(["ns", "cmd"]);

        let cmd = FileCommand {
            files: vec![make_file_spec("missing", "/nonexistent/file.csv", "csv")],
        };

        let result = cmd.execute(&context, &prefix).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("File does not exist"));
    }

    #[tokio::test]
    async fn test_directory_error() {
        init_tracing();
        let context = ExecutionContext::new();
        let prefix = StorePath::from_segments(["ns", "cmd"]);

        let cmd = FileCommand {
            files: vec![make_file_spec(
                "dir",
                fixtures_dir().to_str().unwrap(),
                "csv",
            )],
        };

        let result = cmd.execute(&context, &prefix).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Path is a directory"));
    }

    #[tokio::test]
    async fn test_unsupported_format_error() {
        init_tracing();
        let context = ExecutionContext::new();
        let prefix = StorePath::from_segments(["ns", "cmd"]);

        let cmd = FileCommand {
            files: vec![make_file_spec(
                "users",
                fixtures_dir().join("users.csv").to_str().unwrap(),
                "xml", // unsupported format
            )],
        };

        let result = cmd.execute(&context, &prefix).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unsupported file format: xml"));
    }

    #[tokio::test]
    async fn test_from_attributes() {
        init_tracing();

        let file1 = ObjectBuilder::new()
            .insert("name", "users")
            .insert("file", "/path/to/users.csv")
            .insert("format", "csv")
            .build_scalar();

        let file2 = ObjectBuilder::new()
            .insert("name", "events")
            .insert("file", "/path/to/events.json")
            .insert("format", "json")
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("files", ScalarValue::Array(vec![file1, file2]))
            .build_hashmap();

        let cmd = FileCommand::from_attributes(&attrs).unwrap();

        assert_eq!(cmd.files.len(), 2);
        assert_eq!(cmd.files[0].name, "users");
        assert_eq!(cmd.files[0].file, PathBuf::from("/path/to/users.csv"));
        assert_eq!(cmd.files[0].format, "csv");
        assert_eq!(cmd.files[1].name, "events");
        assert_eq!(cmd.files[1].file, PathBuf::from("/path/to/events.json"));
        assert_eq!(cmd.files[1].format, "json");
    }

    #[tokio::test]
    async fn test_factory_builds_and_executes() {
        init_tracing();

        let attrs = file_attrs(vec![make_file_spec(
            "users",
            fixtures_dir().join("users.csv").to_str().unwrap(),
            "csv",
        )]);

        // Test that factory() returns a valid builder
        let factory = FileCommand::factory();
        let _executable = factory(&attrs).expect("Factory should succeed with valid attributes");

        // Now use Pipeline::execute() pattern to actually run
        let mut pipeline = Pipeline::new();

        pipeline
            .add_namespace(NamespaceBuilder::new("exec"))
            .unwrap()
            .add_command::<FileCommand>("factory_test", &attrs)
            .unwrap();

        let context = pipeline.execute().await.unwrap();

        let (count, total_rows, _) = get_summary(&context, "exec", "factory_test").await;
        assert_eq!(count, 1);
        assert_eq!(total_rows, 3);
    }

    #[tokio::test]
    async fn test_factory_rejects_missing_required_attribute() {
        init_tracing();

        let attrs = Attributes::new();

        let factory = FileCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with missing attribute"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("missing required attribute 'files'"),
                    "Expected missing attribute error, got: {}",
                    msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_factory_rejects_wrong_type() {
        init_tracing();

        let mut attrs = Attributes::new();
        attrs.insert(
            "files".to_string(),
            ScalarValue::String("not an array".to_string()),
        );

        let factory = FileCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with wrong type"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("must be an array"),
                    "Expected type error, got: {}",
                    msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_factory_rejects_invalid_file_object_missing_name() {
        init_tracing();

        // Missing 'name' field
        let file = ObjectBuilder::new()
            .insert("file", "/path/to/file.csv")
            .insert("format", "csv")
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("files", ScalarValue::Array(vec![file]))
            .build_hashmap();

        let factory = FileCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with missing field"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("missing required field 'files[0].name'"),
                    "Expected missing field error, got: {}",
                    msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_factory_rejects_invalid_file_object_missing_file() {
        init_tracing();

        // Missing 'file' field
        let file = ObjectBuilder::new()
            .insert("name", "users")
            .insert("format", "csv")
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("files", ScalarValue::Array(vec![file]))
            .build_hashmap();

        let factory = FileCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with missing field"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("missing required field 'files[0].file'"),
                    "Expected missing field error, got: {}",
                    msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_factory_rejects_invalid_file_object_missing_format() {
        init_tracing();

        // Missing 'format' field
        let file = ObjectBuilder::new()
            .insert("name", "users")
            .insert("file", "/path/to/file.csv")
            .build_scalar();

        let attrs = ObjectBuilder::new()
            .insert("files", ScalarValue::Array(vec![file]))
            .build_hashmap();

        let factory = FileCommand::factory();
        let result = factory(&attrs);

        match result {
            Ok(_) => panic!("Expected factory to fail with missing field"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("missing required field 'files[0].format'"),
                    "Expected missing field error, got: {}",
                    msg
                );
            }
        }
    }
}
