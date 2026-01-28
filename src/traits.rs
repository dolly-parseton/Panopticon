use crate::prelude::*;

pub type CommandFactory = Box<dyn Fn(&Attributes) -> Result<Box<dyn Executable>>>;

pub trait Command: FromAttributes + Descriptor + Executable {}

impl<T: FromAttributes + Descriptor + Executable> Command for T {}

pub trait FromAttributes: Sized {
    fn from_attributes(attrs: &Attributes) -> Result<Self>;

    fn factory() -> CommandFactory
    where
        Self: Executable + Descriptor + 'static,
    {
        Box::new(|attrs| {
            // Validate attributes against schema before parsing
            crate::utils::validate_attributes(attrs, Self::available_attributes())?;
            let instance = Self::from_attributes(attrs)?;
            Ok(Box::new(instance) as Box<dyn Executable>)
        })
    }
}

pub trait Descriptor: Sized {
    fn command_type() -> &'static str;
    fn available_attributes() -> &'static [AttributeSpec<&'static str>];
    fn expected_outputs() -> &'static [ResultSpec<&'static str>];
    // Defaults
    fn required_attributes() -> Vec<&'static AttributeSpec<&'static str>> {
        Self::available_attributes()
            .iter()
            .filter(|attr| attr.required)
            .collect()
    }
    fn optional_attributes() -> Vec<&'static AttributeSpec<&'static str>> {
        Self::available_attributes()
            .iter()
            .filter(|attr| !attr.required)
            .collect()
    }
}

#[async_trait::async_trait]
pub trait Executable: Send + Sync + 'static {
    async fn execute(&self, context: &ExecutionContext, output_prefix: &StorePath) -> Result<()>;
}
