use crate::prelude::*;
use tera::{Map, Number};

pub fn parse_scalar(s: &str) -> ScalarValue {
    match s {
        "true" => ScalarValue::Bool(true),
        "false" => ScalarValue::Bool(false),
        "null" => ScalarValue::Null,
        _ => {
            if let Ok(n) = s.parse::<i64>() {
                ScalarValue::Number(n.into())
            } else if let Ok(n) = s.parse::<f64>() {
                match Number::from_f64(n) {
                    Some(num) => ScalarValue::Number(num),
                    None => ScalarValue::String(s.to_string()),
                }
            } else {
                ScalarValue::String(s.to_string())
            }
        }
    }
}

pub fn is_truthy(value: &ScalarValue) -> bool {
    match value {
        ScalarValue::Null => false,
        ScalarValue::Bool(b) => *b,
        ScalarValue::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        ScalarValue::String(s) => !s.is_empty() && s != "false",
        ScalarValue::Array(a) => !a.is_empty(),
        ScalarValue::Object(o) => !o.is_empty(),
    }
}

pub trait ScalarValueExt {
    fn as_str_or_err(&self, field: &str) -> Result<&str>;
    fn as_i64_or_err(&self, field: &str) -> Result<i64>;
    fn as_f64_or_err(&self, field: &str) -> Result<f64>;
    fn as_bool_or_err(&self, field: &str) -> Result<bool>;
    fn as_array_or_err(&self, field: &str) -> Result<&Vec<ScalarValue>>;
}

impl ScalarValueExt for ScalarValue {
    fn as_str_or_err(&self, field: &str) -> Result<&str> {
        self.as_str()
            .context(format!("'{}' must be a string", field))
    }

    fn as_i64_or_err(&self, field: &str) -> Result<i64> {
        self.as_i64()
            .context(format!("'{}' must be an integer", field))
    }

    fn as_f64_or_err(&self, field: &str) -> Result<f64> {
        self.as_f64()
            .context(format!("'{}' must be a number", field))
    }

    fn as_bool_or_err(&self, field: &str) -> Result<bool> {
        self.as_bool()
            .context(format!("'{}' must be a boolean", field))
    }

    fn as_array_or_err(&self, field: &str) -> Result<&Vec<ScalarValue>> {
        self.as_array()
            .context(format!("'{}' must be an array", field))
    }
}

pub fn validate_value<T: Into<String> + Clone>(
    value: &ScalarValue,
    ty: &TypeDef<T>,
    path: &str,
) -> Result<()> {
    tracing::debug!(path = %path, value_type = ?scalar_type_of(value), "Validating value");

    match ty {
        TypeDef::Scalar(scalar_type) => {
            tracing::debug!(path = %path, expected = ?scalar_type, "Validating scalar");
            validate_scalar(value, scalar_type, path)
        }
        TypeDef::Tabular => {
            tracing::debug!(path = %path, "Rejecting ScalarValue for Tabular type");
            Err(anyhow::anyhow!(
                "'{}' expected Tabular (DataFrame), but got a ScalarValue",
                path
            ))
        }
        TypeDef::ArrayOf(inner_type) => {
            let arr = value
                .as_array()
                .context(format!("'{}' must be an array", path))?;
            tracing::debug!(path = %path, length = arr.len(), "Validating array");
            for (i, item) in arr.iter().enumerate() {
                let item_path = format!("{}[{}]", path, i);
                validate_value(item, inner_type, &item_path)?;
            }
            Ok(())
        }
        TypeDef::ObjectOf { fields } => {
            let obj = value
                .as_object()
                .context(format!("'{}' must be an object", path))?;
            tracing::debug!(path = %path, field_count = fields.len(), "Validating object");
            validate_object(obj, fields, path)
        }
    }
}

fn validate_scalar(value: &ScalarValue, expected: &ScalarType, path: &str) -> Result<()> {
    let actual = scalar_type_of(value);
    if &actual == expected {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "'{}' expected {:?}, got {:?}",
            path,
            expected,
            actual
        ))
    }
}

fn validate_object<T: Into<String> + Clone>(
    obj: &Map<String, ScalarValue>,
    fields: &[FieldSpec<T>],
    path: &str,
) -> Result<()> {
    for field in fields {
        let field_name: String = field.name.clone().into();
        let field_path = if path.is_empty() {
            field_name.clone()
        } else {
            format!("{}.{}", path, field_name)
        };

        match obj.get(&field_name) {
            Some(value) => {
                tracing::debug!(field = %field_path, required = field.required, "Validating object field");
                validate_value(value, &field.ty, &field_path)?;
            }
            None if field.required => {
                tracing::debug!(field = %field_path, "Missing required field");
                return Err(anyhow::anyhow!("missing required field '{}'", field_path));
            }
            None => {
                tracing::debug!(field = %field_path, "Optional field not present, skipping");
            }
        }
    }
    Ok(())
}

#[tracing::instrument(skip(attrs, specs), fields(attr_count = attrs.len(), spec_count = specs.len()))]
pub fn validate_attributes<T: Into<String> + Clone>(
    attrs: &Attributes,
    specs: &[AttributeSpec<T>],
) -> Result<()> {
    tracing::debug!("Starting attribute validation");

    for spec in specs {
        let name: String = spec.name.clone().into();
        match attrs.get(&name) {
            Some(value) => {
                tracing::debug!(attribute = %name, required = spec.required, "Validating attribute");
                validate_value(value, &spec.ty, &name)?;
            }
            None if spec.required => {
                tracing::debug!(attribute = %name, "Missing required attribute");
                return Err(anyhow::anyhow!("missing required attribute '{}'", name));
            }
            None => {
                tracing::debug!(attribute = %name, "Optional attribute not present, skipping");
            }
        }
    }

    tracing::debug!("Attribute validation complete");
    Ok(())
}

#[cfg(test)]
pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .with_test_writer()
        .try_init();
}
