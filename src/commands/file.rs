use crate::imports::*;
use polars::prelude::SerReader;

static FILECOMMAND_SPEC: CommandSchema = LazyLock::new(|| {
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
        .fixed_result(
            "count",
            TypeDef::Scalar(ScalarType::Number),
            Some("The number of files loaded."),
            ResultKind::Meta,
        )
        .fixed_result(
            "total_rows",
            TypeDef::Scalar(ScalarType::Number),
            Some("The total number of rows across all loaded files."),
            ResultKind::Meta,
        )
        .fixed_result(
            "total_size",
            TypeDef::Scalar(ScalarType::Number),
            Some("The total size in bytes across all loaded files."),
            ResultKind::Meta,
        )
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
        let mut total_rows: u64 = 0;
        let mut total_size: u64 = 0;

        for file_spec in self.files.iter() {
            if !file_spec.file.exists() {
                tracing::warn!(missing_file = %file_spec.file.display(), "File does not exist");
                return Err(anyhow::anyhow!(
                    "File does not exist: {}",
                    file_spec.file.display()
                ));
            }
            if file_spec.file.is_dir() {
                tracing::warn!(directory_path = %file_spec.file.display(), "Path is a directory, not a file");
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
    // Going to redo these.
}
