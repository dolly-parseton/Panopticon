use crate::imports::*;

/*
    Types:
    * CommandSpec - Struct representing the specification of a command
*/
pub struct CommandSpec {
    // namespace_index: usize,
    // name: String,
    // attributes: Attributes,
    // builder: CommandFactory,
    // exepected_attributes: Vec<AttributeSpec<String>>,
    // expected_results: Vec<ResultSpec<String>>,
    // dependencies: HashSet<StorePath>,
    pub namespace_index: usize,
    pub name: String,
    pub attributes: Attributes,
    pub builder: CommandFactory,
    pub exepected_attributes: Vec<AttributeSpec<String>>,
    pub expected_results: Vec<ResultSpec<String>>,
    pub dependencies: HashSet<StorePath>,
}

impl CommandSpec {
    pub fn new<T: Command>(namespace_index: usize, name: String, attributes: Attributes) -> Self {
        let dependencies = T::extract_dependencies(&attributes);
        CommandSpec {
            namespace_index,
            name,
            attributes,
            builder: T::factory(),
            exepected_attributes: T::command_attributes()
                .iter()
                .map(|attr| AttributeSpec::<String>::from(attr.clone()))
                .collect(),
            expected_results: T::expected_outputs()
                .iter()
                .map(|res| ResultSpec::<String>::from(res.clone()))
                .collect(),
            dependencies,
        }
    }

    // NOT SURE IF THIS SHOULD BE PUBLIC OR NOT - Todo consider later
    // pub fn index(&self) -> usize {
    //     self.namespace_index
    // }
    // pub fn set_index(&mut self, index: usize) {
    //     self.namespace_index = index;
    // }

    // pub fn name(&self) -> &str {
    //     &self.name
    // }
    // pub fn set_name<T: Into<String>>(&mut self, name: T) {
    //     self.name = name.into();
    // }

    // pub fn attributes(&self) -> &Attributes {
    //     &self.attributes
    // }
    // pub fn expected_attributes(&self) -> &Vec<AttributeSpec<String>> {
    //     &self.exepected_attributes
    // }
    // pub fn expected_results(&self) -> &Vec<ResultSpec<String>> {
    //     &self.expected_results
    // }

    // pub fn dependencies(&self) -> &std::collections::HashSet<StorePath> {
    //     &self.dependencies
    // }
    // pub fn is_dependent_on(&self, path: &StorePath) -> bool {
    //     self.dependencies.contains(path)
    // }
}
