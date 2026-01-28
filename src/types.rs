use crate::prelude::*;

pub type Result<T> = anyhow::Result<T>;

pub use polars::prelude::DataFrame as TabularValue;

pub use tera::Value as ScalarValue;

pub fn scalar_type_of(value: &ScalarValue) -> ScalarType {
    match value {
        ScalarValue::Null => ScalarType::Null,
        ScalarValue::Bool(_) => ScalarType::Bool,
        ScalarValue::Number(_) => ScalarType::Number,
        ScalarValue::String(_) => ScalarType::String,
        ScalarValue::Array(_) => ScalarType::Array,
        ScalarValue::Object(_) => ScalarType::Object,
    }
}

pub fn scalar_value_from<T: serde::Serialize>(input: T) -> Result<ScalarValue> {
    let value = tera::to_value(input)?;
    Ok(value)
}

#[derive(Debug, Clone, PartialEq, Default, Hash, Eq)]
pub enum ScalarType {
    #[default]
    Null,
    Bool,
    Number,
    String,
    Array,
    Object,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResultValue {
    Scalar(ScalarValue),
    Tabular(TabularValue),
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum TypeDef<T: Into<String>> {
    Scalar(ScalarType),
    Tabular,
    ArrayOf(Box<TypeDef<T>>),
    ObjectOf { fields: Vec<FieldSpec<T>> },
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct FieldSpec<T: Into<String>> {
    pub name: T,
    pub ty: TypeDef<T>,
    pub required: bool,
    pub hint: Option<T>,
}

impl From<TypeDef<&'static str>> for TypeDef<String> {
    fn from(td: TypeDef<&'static str>) -> Self {
        match td {
            TypeDef::Scalar(s) => TypeDef::Scalar(s),
            TypeDef::Tabular => TypeDef::Tabular,
            TypeDef::ArrayOf(inner) => TypeDef::ArrayOf(Box::new((*inner).into())),
            TypeDef::ObjectOf { fields } => TypeDef::ObjectOf {
                fields: fields.into_iter().map(|f| f.into()).collect(),
            },
        }
    }
}

impl From<FieldSpec<&'static str>> for FieldSpec<String> {
    fn from(fs: FieldSpec<&'static str>) -> Self {
        FieldSpec {
            name: fs.name.into(),
            ty: fs.ty.into(),
            required: fs.required,
            hint: fs.hint.map(|h| h.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct AttributeSpec<T: Into<String>> {
    pub name: T,
    pub ty: TypeDef<T>,
    pub required: bool,
    pub hint: Option<T>,
    pub default_value: Option<ScalarValue>,
}

impl From<AttributeSpec<&'static str>> for AttributeSpec<String> {
    fn from(attr: AttributeSpec<&'static str>) -> Self {
        AttributeSpec {
            name: attr.name.into(),
            ty: attr.ty.into(),
            required: attr.required,
            hint: attr.hint.map(|h| h.into()),
            default_value: attr.default_value,
        }
    }
}

pub type InputSpec = AttributeSpec<String>;

pub type Attributes = std::collections::HashMap<String, ScalarValue>;

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct ResultSpec<T: Into<String>> {
    pub name: T,
    pub ty: TypeDef<T>,
    pub hint: Option<T>,
}

impl From<ResultSpec<&'static str>> for ResultSpec<String> {
    fn from(attr: ResultSpec<&'static str>) -> Self {
        ResultSpec {
            name: attr.name.into(),
            ty: attr.ty.into(),
            hint: attr.hint.map(|h| h.into()),
        }
    }
}

pub type Results = std::collections::HashMap<String, ResultValue>;

pub struct CommandSpec {
    pub namespace_index: usize,
    pub name: String,
    pub attributes: Attributes,
    pub builder: CommandFactory,
    pub exepected_attributes: Vec<AttributeSpec<String>>,
    pub expected_results: Vec<ResultSpec<String>>,
}

impl CommandSpec {
    pub fn new<T: Command>(namespace_index: usize, name: String, attributes: Attributes) -> Self {
        CommandSpec {
            namespace_index,
            name,
            attributes,
            builder: T::factory(),
            exepected_attributes: T::available_attributes()
                .iter()
                .map(|attr| AttributeSpec::<String>::from(attr.clone()))
                .collect(),
            expected_results: T::expected_outputs()
                .iter()
                .map(|res| ResultSpec::<String>::from(res.clone()))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct StorePath {
    segments: Vec<String>,
}

impl StorePath {
    pub fn from_segments(segments: impl IntoIterator<Item = impl Into<String>>) -> Self {
        StorePath {
            segments: segments.into_iter().map(|s| s.into()).collect(),
        }
    }
    pub fn add_segment(&mut self, segment: impl Into<String>) {
        self.segments.push(segment.into());
    }

    pub fn with_segment(&self, segment: impl Into<String>) -> Self {
        let mut new_path = self.clone();
        new_path.segments.push(segment.into());
        new_path
    }
    pub fn to_dotted(&self) -> String {
        self.segments.join(".")
    }
    pub fn from_dotted(dotted: &str) -> Self {
        let segments = dotted
            .split('.')
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
        StorePath { segments }
    }
    pub fn segments(&self) -> &[String] {
        &self.segments
    }
    pub fn namespace(&self) -> Option<&String> {
        self.segments.first()
    }
    pub fn starts_with(&self, other: &StorePath) -> bool {
        if other.segments.len() > self.segments.len() {
            return false;
        }
        for (a, b) in self.segments.iter().zip(other.segments.iter()) {
            if a != b {
                return false;
            }
        }
        true
    }
    pub fn contains(&self, segment: &str) -> bool {
        self.segments.iter().any(|s| s == segment)
    }
}

impl std::fmt::Display for StorePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_dotted())
    }
}
