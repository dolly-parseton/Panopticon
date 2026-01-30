use crate::imports::*;

/*
    Types:
    * InsertBatch - Struct for batching insert operations into the ExecutionContext
*/
pub struct InsertBatch<'a> {
    context: &'a ExecutionContext,
    prefix: &'a StorePath,
}

impl<'a> InsertBatch<'a> {
    pub fn new(context: &'a ExecutionContext, prefix: &'a StorePath) -> Self {
        Self { context, prefix }
    }

    pub async fn string(&self, segment: &str, value: String) -> Result<()> {
        self.context
            .scalar()
            .insert(
                &self.prefix.with_segment(segment),
                ScalarValue::String(value),
            )
            .await
    }

    pub async fn i64(&self, segment: &str, value: i64) -> Result<()> {
        self.context
            .scalar()
            .insert(
                &self.prefix.with_segment(segment),
                ScalarValue::Number(value.into()),
            )
            .await
    }

    pub async fn f64(&self, segment: &str, value: f64) -> Result<()> {
        self.context
            .scalar()
            .insert(
                &self.prefix.with_segment(segment),
                match tera::Number::from_f64(value) {
                    Some(num) => ScalarValue::Number(num),
                    None => {
                        return Err(anyhow::anyhow!(
                            "Cannot insert NaN or infinite f64 value at '{}'",
                            self.prefix.with_segment(segment)
                        ));
                    }
                },
            )
            .await
    }

    pub async fn u64(&self, segment: &str, value: u64) -> Result<()> {
        self.context
            .scalar()
            .insert(
                &self.prefix.with_segment(segment),
                ScalarValue::Number(value.into()),
            )
            .await
    }

    pub async fn bool(&self, segment: &str, value: bool) -> Result<()> {
        self.context
            .scalar()
            .insert(&self.prefix.with_segment(segment), ScalarValue::Bool(value))
            .await
    }

    pub async fn null(&self, segment: &str) -> Result<()> {
        self.context
            .scalar()
            .insert(&self.prefix.with_segment(segment), ScalarValue::Null)
            .await
    }

    pub async fn scalar(&self, segment: &str, value: ScalarValue) -> Result<()> {
        self.context
            .scalar()
            .insert(&self.prefix.with_segment(segment), value)
            .await
    }

    pub async fn tabular(&self, segment: &str, tabular: TabularValue) -> Result<()> {
        self.context
            .tabular()
            .insert(&self.prefix.with_segment(segment), tabular)
            .await
    }
}

/*
    Helper functions: - TODO, decide if these should be exports or stay pub(crate)
    * insert_at_path - Inserts a ScalarValue at the specified StorePath within a root ScalarValue
    * get_at_path - Retrieves a reference to a ScalarValue at the specified StorePath within a root ScalarValue
    * scalar_type_of - Returns the ScalarType of a given ScalarValue
    * parse_scalar - Parses a &str into a ScalarValue
    * is_truthy - Determines the truthiness of a ScalarValue (similar to JavaScript truthiness, couldn't think of a better name lol)
    * to_scalar - Module with helper functions to create ScalarValues of various types
*/
pub(crate) fn insert_at_path(
    root: &mut ScalarValue,
    path: &StorePath,
    value: ScalarValue,
) -> Result<()> {
    let segments = &path.segments();
    if segments.is_empty() {
        return Err(anyhow::anyhow!("StorePath has no segments"));
    }

    let mut current = root;
    // We skip the first segment (namespace key), so we iterate over segments[1..]
    // The last segment index after skip is (segments.len() - 1) - 1 = segments.len() - 2
    let last_index = segments.len().saturating_sub(2);

    for (i, segment) in segments.iter().skip(1).enumerate() {
        match current {
            ScalarValue::Object(map) => {
                if i == last_index {
                    map.insert(segment.clone(), value);
                    return Ok(());
                } else {
                    current = map
                        .entry(segment.clone())
                        .or_insert_with(|| ObjectBuilder::new().build_scalar());
                }
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Cannot insert into non-object ScalarValue at segment '{}'",
                    segment
                ));
            }
        }
    }

    Ok(())
}

pub(in crate::values) fn get_at_path<'a>(
    root: &'a ScalarValue,
    path: &StorePath,
) -> Option<&'a ScalarValue> {
    let segments = &path.segments();
    if segments.is_empty() {
        return None;
    }

    let mut current = root;

    // Skip the first segment since it's the namespace
    for segment in segments.iter().skip(1) {
        match current {
            ScalarValue::Object(map) => {
                if let Some(next) = map.get(segment) {
                    current = next;
                } else {
                    return None;
                }
            }
            _ => {
                return None;
            }
        }
    }

    Some(current)
}

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

pub fn parse_scalar(s: &str) -> ScalarValue {
    match s {
        "true" => ScalarValue::Bool(true),
        "false" => ScalarValue::Bool(false),
        "null" => ScalarValue::Null,
        _ => {
            if let Ok(n) = s.parse::<i64>() {
                ScalarValue::Number(n.into())
            } else if let Ok(n) = s.parse::<f64>() {
                match tera::Number::from_f64(n) {
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

pub mod to_scalar {
    use crate::imports::*;
    /*
        Functions:
        * string - Creates a ScalarValue::String
        * i64 - Creates a ScalarValue::Number from i64
        * f64 - Creates a ScalarValue::Number from f64
        * u64 - Creates a ScalarValue::Number from u64
        * bool - Creates a ScalarValue::Bool
        * null - Creates a ScalarValue::Null
        * array - Creates a ScalarValue::Array
        * object - Creates a ScalarValue::Object
    */

    pub fn string<T: Into<String>>(s: T) -> ScalarValue {
        ScalarValue::String(s.into())
    }
    pub fn i64(n: i64) -> ScalarValue {
        ScalarValue::Number(n.into())
    }
    pub fn f64(n: f64) -> ScalarValue {
        match tera::Number::from_f64(n) {
            Some(num) => ScalarValue::Number(num),
            None => ScalarValue::String(n.to_string()),
        }
    }
    pub fn u64(n: u64) -> ScalarValue {
        ScalarValue::Number(n.into())
    }
    pub fn bool(b: bool) -> ScalarValue {
        ScalarValue::Bool(b)
    }
    pub fn null() -> ScalarValue {
        ScalarValue::Null
    }
    pub fn array(arr: Vec<ScalarValue>) -> ScalarValue {
        ScalarValue::Array(arr)
    }
    pub fn object(obj: tera::Map<String, ScalarValue>) -> ScalarValue {
        ScalarValue::Object(obj)
    }
}
