use crate::imports::*;

/*
    Types:
    * CommandSpec - Struct representing the specification of a command
*/
pub struct CommandSpec {
    pub namespace_index: usize,
    pub name: String,
    pub command_type: String,
    pub attributes: Attributes,
    pub builder: CommandFactory,
    pub exepected_attributes: Vec<AttributeSpec<String>>,
    pub expected_results: Vec<ResultSpec<String>>,
    pub dependencies: HashSet<StorePath>,
    pub provides_extensions: Vec<ExtensionKey>,
    pub requires_extensions: Vec<ExtensionKey>,
}

impl CommandSpec {
    pub fn new<T: Command>(
        namespace_index: usize,
        name: String,
        attributes: Attributes,
    ) -> Result<Self> {
        let dependencies = T::extract_dependencies(&attributes)?;
        Ok(CommandSpec {
            namespace_index,
            name,
            command_type: T::command_type().to_string(),
            attributes,
            builder: T::factory(),
            exepected_attributes: T::available_attributes()
                .into_iter()
                .map(|attr| AttributeSpec::<String>::from(attr.clone()))
                .collect(),
            expected_results: T::available_results()
                .into_iter()
                .map(|res| ResultSpec::<String>::from(res.clone()))
                .collect(),
            dependencies,
            provides_extensions: T::provides_extensions().to_vec(),
            requires_extensions: T::requires_extensions().to_vec(),
        })
    }

    pub(crate) fn validate_attributes(&self) -> Result<()> {
        use crate::pipeline::validation::validate_attributes;
        validate_attributes(&self.attributes, &self.exepected_attributes)
    }
}
