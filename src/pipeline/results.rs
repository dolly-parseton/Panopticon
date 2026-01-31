use crate::imports::*;
use polars::prelude::SerWriter;

/*
    CONSTS:
    * DEFAULT_OUTPUT_DIRNAME - Default directory for storing command results
*/
pub const DEFAULT_OUTPUT_DIRNAME: &str = "panopticon_results";

/*
    Types:
    * ResultSettings - Settings for how to output results (file path, format, exclusions)
    * ResultStore - Store of all command results after pipeline execution
    * CommandResults - Results produced by a single command, including metadata and actual data
    * ResultValue - Enum representing either a scalar result or a tabular result with associated metadata
    * TabularFormat - Enum representing supported tabular data formats (CSV, Parquet, JSON)
*/
#[derive(Debug, Clone)]
pub struct ResultSettings {
    pub(crate) output_path: PathBuf,
    pub(crate) format: TabularFormat,
    pub(crate) excluded_commands: Vec<StorePath>, // Excludes all result fields from these commands (validated in runtime against CommandSpec)
}

// Builder methods
impl ResultSettings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn output_path(&self) -> &PathBuf {
        &self.output_path
    }

    pub fn format(&self) -> &TabularFormat {
        &self.format
    }

    pub fn excluded_commands(&self) -> impl Iterator<Item = &StorePath> {
        self.excluded_commands.iter()
    }

    pub fn with_output_path(mut self, path: PathBuf) -> Self {
        self.output_path = path;
        self
    }

    pub fn with_format(mut self, format: TabularFormat) -> Self {
        self.format = format;
        self
    }

    pub fn with_excluded_commands(mut self, excluded: Vec<StorePath>) -> Self {
        self.excluded_commands = excluded;
        self
    }
}

impl Default for ResultSettings {
    fn default() -> Self {
        ResultSettings {
            // If it fails so be it
            output_path: std::env::current_dir()
                .context("Failed to get current directory when creating ResultSettings")
                .unwrap()
                .join(DEFAULT_OUTPUT_DIRNAME),
            format: TabularFormat::Json,
            excluded_commands: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResultStore {
    pub(crate) results: Vec<CommandResults>,
}

impl ResultStore {
    pub fn iter(&self) -> impl Iterator<Item = &CommandResults> {
        self.results.iter()
    }

    pub fn len(&self) -> usize {
        self.results.len()
    }

    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    pub fn get_by_source(&self, source: &StorePath) -> Option<&CommandResults> {
        self.results.iter().find(|r| &r.source == source)
    }
}

#[derive(Debug, Clone)]
pub struct CommandResults {
    pub(crate) source: StorePath, // Path where the command results are stored (doesn't include actual field names)
    pub(crate) meta: HashMap<StorePath, ScalarValue>, // Metadata about the results, indexed by the full StorePath
    pub(crate) data: HashMap<StorePath, ResultValue>, // Actual result values, indexed by the full StorePath
}

impl CommandResults {
    pub fn source(&self) -> &StorePath {
        &self.source
    }

    pub fn meta_get(&self, key: &StorePath) -> Option<&ScalarValue> {
        self.meta.get(key)
    }

    pub fn data_get(&self, key: &StorePath) -> Option<&ResultValue> {
        self.data.get(key)
    }

    pub fn meta_keys(&self) -> impl Iterator<Item = &StorePath> {
        self.meta.keys()
    }

    pub fn data_keys(&self) -> impl Iterator<Item = &StorePath> {
        self.data.keys()
    }

    pub fn meta_iter(&self) -> impl Iterator<Item = (&StorePath, &ScalarValue)> {
        self.meta.iter()
    }

    pub fn data_iter(&self) -> impl Iterator<Item = (&StorePath, &ResultValue)> {
        self.data.iter()
    }
}

#[derive(Debug, Clone)]
pub enum ResultValue {
    Scalar {
        ty: ScalarType,
        value: ScalarValue,
    },
    Tabular {
        path: PathBuf,
        format: TabularFormat,
        rows_count: usize,
        columns_count: usize,
    },
}

impl ResultValue {
    pub fn as_scalar(&self) -> Option<(&ScalarType, &ScalarValue)> {
        match self {
            ResultValue::Scalar { ty, value } => Some((ty, value)),
            _ => None,
        }
    }

    pub fn as_tabular(&self) -> Option<(&PathBuf, &TabularFormat, usize, usize)> {
        match self {
            ResultValue::Tabular {
                path,
                format,
                rows_count,
                columns_count,
            } => Some((path, format, *rows_count, *columns_count)),
            _ => None,
        }
    }

    pub fn is_scalar(&self) -> bool {
        matches!(self, ResultValue::Scalar { .. })
    }
    pub fn is_tabular(&self) -> bool {
        matches!(self, ResultValue::Tabular { .. })
    }
}

#[derive(Debug, Clone)]
pub enum TabularFormat {
    Csv,
    Parquet,
    Json,
}

impl TabularFormat {
    pub fn extension(&self) -> &str {
        match self {
            TabularFormat::Csv => "csv",
            TabularFormat::Parquet => "parquet",
            TabularFormat::Json => "json",
        }
    }
}

pub(crate) fn write_tabular(
    df: &TabularValue,
    path: &PathBuf,
    format: &TabularFormat,
) -> Result<()> {
    let mut df = df.clone();
    match format {
        TabularFormat::Csv => {
            let file = std::fs::File::create(path)?;
            polars::prelude::CsvWriter::new(file)
                .finish(&mut df)
                .context("Failed to write CSV")?;
        }
        TabularFormat::Json => {
            let file = std::fs::File::create(path)?;
            polars::prelude::JsonWriter::new(file)
                .finish(&mut df)
                .context("Failed to write JSON")?;
        }
        TabularFormat::Parquet => {
            let file = std::fs::File::create(path)?;
            polars::prelude::ParquetWriter::new(file)
                .finish(&mut df)
                .context("Failed to write Parquet")?;
        }
    }
    Ok(())
}
