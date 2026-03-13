use crate::imports::*;
use std::fmt;
use serde::de::IntoDeserializer;

fn str_deserializer(s: &str) -> serde::de::value::StrDeserializer<'_, DeserializeError> {
    s.into_deserializer()
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct DeserializeError(String);

impl fmt::Display for DeserializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DeserializeError {}

impl serde::de::Error for DeserializeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        DeserializeError(msg.to_string())
    }
}

// ---------------------------------------------------------------------------
// StoreEntryDeserializer — core Deserializer for a single &StoreEntry
// ---------------------------------------------------------------------------

struct StoreEntryDeserializer<'de> {
    entry: &'de StoreEntry,
}

impl<'de> serde::Deserializer<'de> for StoreEntryDeserializer<'de> {
    type Error = DeserializeError;

    fn deserialize_any<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.entry {
            StoreEntry::Var { value, .. } => match value {
                Value::Null => visitor.visit_unit(),
                Value::Boolean(b) => visitor.visit_bool(*b),
                Value::Integer(i) => visitor.visit_i64(*i),
                Value::Float(f) => visitor.visit_f64(*f),
                Value::Text(s) => visitor.visit_str(s),
            },
            StoreEntry::Array(items) => visitor.visit_seq(StoreEntrySeqAccess {
                iter: items.iter(),
            }),
            StoreEntry::Map(entries) => visitor.visit_map(StoreEntryMapAccess {
                iter: entries.iter(),
                current_value: None,
            }),
        }
    }

    fn deserialize_option<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.entry {
            StoreEntry::Var {
                value: Value::Null, ..
            } => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_newtype_struct<V: serde::de::Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: serde::de::Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.entry {
            StoreEntry::Var {
                value: Value::Text(s),
                ..
            } => visitor.visit_enum(str_deserializer(s.as_str())),
            StoreEntry::Map(entries) => {
                visitor.visit_enum(StoreEntryEnumAccess { iter: entries.iter() })
            }
            _ => Err(serde::de::Error::custom(
                "expected a string or map for enum deserialization",
            )),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64
        char str string bytes byte_buf unit unit_struct
        seq tuple tuple_struct map struct identifier ignored_any
    }
}

// ---------------------------------------------------------------------------
// EnumAccess — for externally-tagged enum deserialization from a Map
// ---------------------------------------------------------------------------

struct StoreEntryEnumAccess<'de> {
    iter: std::collections::hash_map::Iter<'de, String, StoreEntry>,
}

impl<'de> serde::de::EnumAccess<'de> for StoreEntryEnumAccess<'de> {
    type Error = DeserializeError;
    type Variant = StoreEntryVariantAccess<'de>;

    fn variant_seed<V: serde::de::DeserializeSeed<'de>>(
        mut self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Self::Error> {
        let (key, value) = self.iter.next().ok_or_else(|| {
            serde::de::Error::custom("expected a single-entry map for enum deserialization")
        })?;
        let variant = seed.deserialize(str_deserializer(key.as_str()))?;
        Ok((variant, StoreEntryVariantAccess { value }))
    }
}

struct StoreEntryVariantAccess<'de> {
    value: &'de StoreEntry,
}

impl<'de> serde::de::VariantAccess<'de> for StoreEntryVariantAccess<'de> {
    type Error = DeserializeError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T: serde::de::DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, Self::Error> {
        seed.deserialize(StoreEntryDeserializer { entry: self.value })
    }

    fn tuple_variant<V: serde::de::Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        serde::Deserializer::deserialize_seq(
            StoreEntryDeserializer { entry: self.value },
            visitor,
        )
    }

    fn struct_variant<V: serde::de::Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        serde::Deserializer::deserialize_map(
            StoreEntryDeserializer { entry: self.value },
            visitor,
        )
    }
}

// ---------------------------------------------------------------------------
// SeqAccess — for Array deserialization
// ---------------------------------------------------------------------------

struct StoreEntrySeqAccess<'de> {
    iter: std::slice::Iter<'de, StoreEntry>,
}

impl<'de> serde::de::SeqAccess<'de> for StoreEntrySeqAccess<'de> {
    type Error = DeserializeError;

    fn next_element_seed<T: serde::de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Self::Error> {
        match self.iter.next() {
            Some(entry) => seed
                .deserialize(StoreEntryDeserializer { entry })
                .map(Some),
            None => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// MapAccess — for Map deserialization
// ---------------------------------------------------------------------------

struct StoreEntryMapAccess<'de> {
    iter: std::collections::hash_map::Iter<'de, String, StoreEntry>,
    current_value: Option<&'de StoreEntry>,
}

impl<'de> serde::de::MapAccess<'de> for StoreEntryMapAccess<'de> {
    type Error = DeserializeError;

    fn next_key_seed<K: serde::de::DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        match self.iter.next() {
            Some((key, value)) => {
                self.current_value = Some(value);
                seed.deserialize(str_deserializer(key.as_str())).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: serde::de::DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        let entry = self
            .current_value
            .take()
            .expect("next_value_seed called before next_key_seed");
        seed.deserialize(StoreEntryDeserializer { entry })
    }
}

// ---------------------------------------------------------------------------
// PrefixMapDeserializer — deserializes a filtered set of store entries as a map
// ---------------------------------------------------------------------------

struct PrefixMapDeserializer<'de> {
    entries: Vec<(&'de str, &'de StoreEntry)>,
}

impl<'de> serde::Deserializer<'de> for PrefixMapDeserializer<'de> {
    type Error = DeserializeError;

    fn deserialize_any<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_map<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_map(PrefixMapAccess {
            iter: self.entries.into_iter(),
            current_value: None,
        })
    }

    fn deserialize_struct<V: serde::de::Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_map(visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64
        char str string bytes byte_buf option unit unit_struct
        newtype_struct seq tuple tuple_struct enum identifier ignored_any
    }
}

struct PrefixMapAccess<'de> {
    iter: std::vec::IntoIter<(&'de str, &'de StoreEntry)>,
    current_value: Option<&'de StoreEntry>,
}

impl<'de> serde::de::MapAccess<'de> for PrefixMapAccess<'de> {
    type Error = DeserializeError;

    fn next_key_seed<K: serde::de::DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        match self.iter.next() {
            Some((key, value)) => {
                self.current_value = Some(value);
                seed.deserialize(str_deserializer(key)).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: serde::de::DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        let entry = self
            .current_value
            .take()
            .expect("next_value_seed called before next_key_seed");
        seed.deserialize(StoreEntryDeserializer { entry })
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn from_prefix_entries<'de, T: serde::Deserialize<'de>>(
    entries: impl Iterator<Item = (&'de str, &'de StoreEntry)>,
) -> Result<T, DeserializeError> {
    let entries: Vec<_> = entries.collect();
    if entries.is_empty() {
        return Err(DeserializeError(
            "no entries found for the given prefix".into(),
        ));
    }
    T::deserialize(PrefixMapDeserializer { entries })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Type, Value};
    use serde::Deserialize;
    use std::collections::HashMap;

    // Helper to deserialize a single StoreEntry
    fn from_entry<'de, T: Deserialize<'de>>(entry: &'de StoreEntry) -> Result<T, DeserializeError> {
        T::deserialize(StoreEntryDeserializer { entry })
    }

    // -- Scalar tests --

    #[test]
    fn deserialize_bool() {
        let entry = StoreEntry::Var {
            value: Value::Boolean(true),
            ty: Type::Boolean,
        };
        assert_eq!(from_entry::<bool>(&entry).unwrap(), true);
    }

    #[test]
    fn deserialize_i64() {
        let entry = StoreEntry::Var {
            value: Value::Integer(42),
            ty: Type::Integer,
        };
        assert_eq!(from_entry::<i64>(&entry).unwrap(), 42);
    }

    #[test]
    fn deserialize_i32_from_integer() {
        let entry = StoreEntry::Var {
            value: Value::Integer(42),
            ty: Type::Integer,
        };
        assert_eq!(from_entry::<i32>(&entry).unwrap(), 42);
    }

    #[test]
    fn deserialize_f64() {
        let entry = StoreEntry::Var {
            value: Value::Float(3.14),
            ty: Type::Float,
        };
        assert_eq!(from_entry::<f64>(&entry).unwrap(), 3.14);
    }

    #[test]
    fn deserialize_string() {
        let entry = StoreEntry::Var {
            value: Value::Text("hello".into()),
            ty: Type::Text,
        };
        assert_eq!(from_entry::<String>(&entry).unwrap(), "hello");
    }

    // -- Option tests --

    #[test]
    fn deserialize_option_none() {
        let entry = StoreEntry::Var {
            value: Value::Null,
            ty: Type::Null,
        };
        assert_eq!(from_entry::<Option<String>>(&entry).unwrap(), None);
    }

    #[test]
    fn deserialize_option_some() {
        let entry = StoreEntry::Var {
            value: Value::Text("present".into()),
            ty: Type::Text,
        };
        assert_eq!(
            from_entry::<Option<String>>(&entry).unwrap(),
            Some("present".into())
        );
    }

    // -- Collection tests --

    #[test]
    fn deserialize_vec() {
        let entry = StoreEntry::Array(vec![
            StoreEntry::Var {
                value: Value::Integer(1),
                ty: Type::Integer,
            },
            StoreEntry::Var {
                value: Value::Integer(2),
                ty: Type::Integer,
            },
            StoreEntry::Var {
                value: Value::Integer(3),
                ty: Type::Integer,
            },
        ]);
        assert_eq!(from_entry::<Vec<i64>>(&entry).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn deserialize_hashmap() {
        let mut map = HashMap::new();
        map.insert(
            "a".to_string(),
            StoreEntry::Var {
                value: Value::Integer(1),
                ty: Type::Integer,
            },
        );
        map.insert(
            "b".to_string(),
            StoreEntry::Var {
                value: Value::Integer(2),
                ty: Type::Integer,
            },
        );
        let entry = StoreEntry::Map(map);
        let result: HashMap<String, i64> = from_entry(&entry).unwrap();
        assert_eq!(result.get("a"), Some(&1));
        assert_eq!(result.get("b"), Some(&2));
    }

    // -- Struct tests --

    #[test]
    fn deserialize_struct_from_map() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Person {
            name: String,
            age: i64,
            active: bool,
        }

        let mut map = HashMap::new();
        map.insert(
            "name".to_string(),
            StoreEntry::Var {
                value: Value::Text("Alice".into()),
                ty: Type::Text,
            },
        );
        map.insert(
            "age".to_string(),
            StoreEntry::Var {
                value: Value::Integer(30),
                ty: Type::Integer,
            },
        );
        map.insert(
            "active".to_string(),
            StoreEntry::Var {
                value: Value::Boolean(true),
                ty: Type::Boolean,
            },
        );
        let entry = StoreEntry::Map(map);
        let person: Person = from_entry(&entry).unwrap();
        assert_eq!(
            person,
            Person {
                name: "Alice".into(),
                age: 30,
                active: true,
            }
        );
    }

    // -- Nested tests --

    #[test]
    fn deserialize_vec_of_structs() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Item {
            label: String,
            count: i64,
        }

        let entry = StoreEntry::Array(vec![
            StoreEntry::Map(HashMap::from([
                (
                    "label".to_string(),
                    StoreEntry::Var {
                        value: Value::Text("first".into()),
                        ty: Type::Text,
                    },
                ),
                (
                    "count".to_string(),
                    StoreEntry::Var {
                        value: Value::Integer(10),
                        ty: Type::Integer,
                    },
                ),
            ])),
            StoreEntry::Map(HashMap::from([
                (
                    "label".to_string(),
                    StoreEntry::Var {
                        value: Value::Text("second".into()),
                        ty: Type::Text,
                    },
                ),
                (
                    "count".to_string(),
                    StoreEntry::Var {
                        value: Value::Integer(20),
                        ty: Type::Integer,
                    },
                ),
            ])),
        ]);

        let items: Vec<Item> = from_entry(&entry).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label, "first");
        assert_eq!(items[0].count, 10);
        assert_eq!(items[1].label, "second");
        assert_eq!(items[1].count, 20);
    }

    // -- Prefix map tests (simulates Complete::returns) --

    #[test]
    fn deserialize_from_prefix_entries() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Output {
            severity: String,
            score: f64,
            enriched: bool,
        }

        let severity = StoreEntry::Var {
            value: Value::Text("high".into()),
            ty: Type::Text,
        };
        let score = StoreEntry::Var {
            value: Value::Float(9.5),
            ty: Type::Float,
        };
        let enriched = StoreEntry::Var {
            value: Value::Boolean(true),
            ty: Type::Boolean,
        };

        let entries = vec![
            ("severity", &severity),
            ("score", &score),
            ("enriched", &enriched),
        ];

        let output: Output = from_prefix_entries(entries.into_iter()).unwrap();
        assert_eq!(
            output,
            Output {
                severity: "high".into(),
                score: 9.5,
                enriched: true,
            }
        );
    }

    #[test]
    fn deserialize_prefix_entries_empty() {
        let entries: Vec<(&str, &StoreEntry)> = vec![];
        let result = from_prefix_entries::<HashMap<String, String>>(entries.into_iter());
        assert!(result.is_err());
    }

    // -- Error tests --

    #[test]
    fn type_mismatch_error() {
        let entry = StoreEntry::Var {
            value: Value::Boolean(true),
            ty: Type::Boolean,
        };
        let result = from_entry::<i64>(&entry);
        assert!(result.is_err());
    }
}
