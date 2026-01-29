use crate::imports::*;

/*
    Functions:
    (PUBLIC METHOD)
    * validate_attributes - Validates a set of Attributes against a list of AttributeSpecs
    (PRIVATE METHODS)
    * validate_value - Validates a ScalarValue against a TypeDef
    * validate_scalar - Validates a ScalarValue against a ScalarType
    * validate_object - Validates a ScalarValue object against a list of FieldSpecs
*/
#[tracing::instrument(skip(attrs, specs), fields(attr_count = attrs.len()))]
pub fn validate_attributes<'a, T, I>(attrs: &Attributes, specs: I) -> Result<()>
where
    T: Into<String> + Clone + 'a,
    I: IntoIterator<Item = &'a AttributeSpec<T>>,
{
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
    obj: &tera::Map<String, ScalarValue>,
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
