use std::hash::Hash;

use crate::imports::*;

mod sealed {
    #[doc(hidden)]
    pub struct New(());
    #[doc(hidden)]
    pub struct InProgress(());
    #[doc(hidden)]
    pub struct Failed(());
    #[doc(hidden)]
    pub struct Completed(());
}

/*
    CONSTS:
    * DEFAULT_OUTPUT_DIRNAME - Default directory for storing command results
*/
pub const DEFAULT_OUTPUT_DIRNAME: &str = "panopticon_results";

/*
    Types: TODO, extend result store as a product of ExecutionContext after pipeline execution finishes. It'll need several 'save to file' and display methods.
    * ExecutionContext - Holds the scalar and tabular stores for command execution context
    * ResultSettings - Settings for how to output results (file path, format, exclusions)
    * ResultStore - Store of all command results after pipeline execution
    * CommandResults - Results produced by a single command, including metadata and actual data
    * ResultValue - Enum representing either a scalar result or a tabular result with associated metadata
    * TabularFormat - Enum representing supported tabular data formats (CSV, Parquet, JSON)
*/

#[derive(Clone, Debug, Default)]
pub struct ExecutionContext {
    scalar_store: ScalarStore,
    tabular_store: TabularStore,
}

impl ExecutionContext {
    pub fn new() -> Self {
        ExecutionContext {
            scalar_store: ScalarStore::new(),
            tabular_store: TabularStore::new(),
        }
    }

    pub fn scalar(&self) -> &ScalarStore {
        &self.scalar_store
    }

    pub fn tabular(&self) -> &TabularStore {
        &self.tabular_store
    }

    pub async fn substitute<T: Into<String>>(&self, template: T) -> Result<String> {
        self.scalar_store.render_template(template.into()).await
    }

    // pub fn results(self) -> Result<ResultStore> {
    //     todo!()
    // }
}

// #[derive(Debug, Clone)]
// pub struct ResultSettings {
//     pub output_path: PathBuf,
//     pub format: TabularFormat,
//     pub exclude: Vec<StorePath>,
// }

// impl Default for ResultSettings {
//     fn default() -> Self {
//         ResultSettings {
//             output_path: env::current_dir()?.join(DEFAULT_OUTPUT_DIRNAME),
//             format: TabularFormat::Json,
//             exclude: vec![],
//         }
//     }
// }

// #[derive(Debug, Clone)]
// pub struct ResultStore {
//     results: Vec<CommandResults>,
// }

// impl ResultStore {
//     pub fn from_context(
//         context: ExecutionContext,
//         // Not used this pattern before, experimenting with destructuring in function parameters, might apply elsewhere. How does this work with borrows?
//         ResultSettings {
//             output_path,
//             format,
//             exclude,
//         }: ResultSettings,
//     ) -> Result<Self> {

//     }
// }

// #[derive(Debug, Clone)]
// pub struct CommandResults {
//     source: StorePath, // Path where the command results are stored (doesn't include actual field names)
//     meta: HashMap<StorePath, ScalarValue>, // Metadata about the results, indexed by the full StorePath
//     data: HashMap<StorePath, ResultValue>, // Actual result values, indexed by the full StorePath
// }

// #[derive(Debug, Clone)]
// pub enum ResultValue {
//     Scalar {
//         ty: ScalarType,
//         value: ScalarValue,
//     },
//     Tabular {
//         path: PathBuf,
//         format: TabularFormat,
//         rows_count: usize,
//         columns_count: usize,
//     },
// }

// #[derive(Debug, Clone)]
// pub enum TabularFormat {
//     Csv,
//     Parquet,
//     Json,
// }
