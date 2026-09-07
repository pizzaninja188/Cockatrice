//! Canonical, lossless JSON for diagnostic state only. Composite map keys stay structured;
//! unordered maps/sets are sorted. No deserializer exists: replay always executes commands.

use serde::ser::{
    self, Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
    SerializeTuple, SerializeTupleStruct, SerializeTupleVariant,
};
use serde_json::{Map, Value};

pub fn to_value<T: Serialize + ?Sized>(value: &T) -> Result<Value, serde_json::Error> {
    value.serialize(Serializer)
}

pub(crate) fn serialize_set<T: Serialize + Eq + std::hash::Hash, S: ser::Serializer>(
    set: &std::collections::HashSet<T>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut values: Vec<_> = set
        .iter()
        .map(to_value)
        .collect::<Result<_, _>>()
        .map_err(ser::Error::custom)?;
    values.sort_by_cached_key(Value::to_string);
    values.serialize(serializer)
}

struct Serializer;
struct Sequence {
    values: Vec<Value>,
    variant: Option<&'static str>,
}
struct Object {
    fields: Map<String, Value>,
    variant: Option<&'static str>,
}
struct Mapping {
    entries: Vec<(Value, Value)>,
    key: Option<Value>,
}

fn variant(name: Option<&str>, value: Value) -> Value {
    match name {
        None => value,
        Some(name) => serde_json::json!({"variant": name, "value": value}),
    }
}

macro_rules! number {
    ($method:ident, $ty:ty) => {
        fn $method(self, v: $ty) -> Result<Value, Self::Error> {
            Ok(Value::from(v))
        }
    };
}
macro_rules! decimal {
    ($method:ident, $ty:ty) => {
        fn $method(self, v: $ty) -> Result<Value, Self::Error> {
            Ok(Value::String(v.to_string()))
        }
    };
}

impl ser::Serializer for Serializer {
    type Ok = Value;
    type Error = serde_json::Error;
    type SerializeSeq = Sequence;
    type SerializeTuple = Sequence;
    type SerializeTupleStruct = Sequence;
    type SerializeTupleVariant = Sequence;
    type SerializeMap = Mapping;
    type SerializeStruct = Object;
    type SerializeStructVariant = Object;
    number!(serialize_bool, bool);
    number!(serialize_i8, i8);
    number!(serialize_i16, i16);
    number!(serialize_i32, i32);
    number!(serialize_u8, u8);
    number!(serialize_u16, u16);
    number!(serialize_u32, u32);
    decimal!(serialize_i64, i64);
    decimal!(serialize_u64, u64);
    decimal!(serialize_i128, i128);
    decimal!(serialize_u128, u128);
    fn serialize_f32(self, v: f32) -> Result<Value, Self::Error> {
        self.serialize_f64(f64::from(v))
    }
    fn serialize_f64(self, v: f64) -> Result<Value, Self::Error> {
        Ok(if v.is_finite() {
            Value::from(v)
        } else {
            Value::String(v.to_string())
        })
    }
    fn serialize_char(self, v: char) -> Result<Value, Self::Error> {
        Ok(Value::String(v.to_string()))
    }
    fn serialize_str(self, v: &str) -> Result<Value, Self::Error> {
        Ok(Value::String(v.into()))
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<Value, Self::Error> {
        Ok(Value::Array(v.iter().copied().map(Value::from).collect()))
    }
    fn serialize_none(self) -> Result<Value, Self::Error> {
        Ok(Value::Null)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<Value, Self::Error> {
        to_value(v)
    }
    fn serialize_unit(self) -> Result<Value, Self::Error> {
        Ok(Value::Null)
    }
    fn serialize_unit_struct(self, name: &'static str) -> Result<Value, Self::Error> {
        Ok(Value::String(name.into()))
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        name: &'static str,
    ) -> Result<Value, Self::Error> {
        Ok(Value::String(name.into()))
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        v: &T,
    ) -> Result<Value, Self::Error> {
        to_value(v)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        name: &'static str,
        v: &T,
    ) -> Result<Value, Self::Error> {
        Ok(variant(Some(name), to_value(v)?))
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Sequence, Self::Error> {
        Ok(Sequence {
            values: vec![],
            variant: None,
        })
    }
    fn serialize_tuple(self, len: usize) -> Result<Sequence, Self::Error> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_struct(self, _: &'static str, len: usize) -> Result<Sequence, Self::Error> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        name: &'static str,
        _: usize,
    ) -> Result<Sequence, Self::Error> {
        Ok(Sequence {
            values: vec![],
            variant: Some(name),
        })
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Mapping, Self::Error> {
        Ok(Mapping {
            entries: vec![],
            key: None,
        })
    }
    fn serialize_struct(self, name: &'static str, _: usize) -> Result<Object, Self::Error> {
        let mut fields = Map::new();
        fields.insert("_type".into(), Value::String(name.into()));
        Ok(Object {
            fields,
            variant: None,
        })
    }
    fn serialize_struct_variant(
        self,
        name: &'static str,
        _: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Object, Self::Error> {
        let mut object = self.serialize_struct(name, len)?;
        object.variant = Some(variant);
        Ok(object)
    }
}

impl SerializeSeq for Sequence {
    type Ok = Value;
    type Error = serde_json::Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Self::Error> {
        self.values.push(to_value(v)?);
        Ok(())
    }
    fn end(self) -> Result<Value, Self::Error> {
        Ok(variant(self.variant, Value::Array(self.values)))
    }
}
impl SerializeTuple for Sequence {
    type Ok = Value;
    type Error = serde_json::Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Self::Error> {
        SerializeSeq::serialize_element(self, v)
    }
    fn end(self) -> Result<Value, Self::Error> {
        SerializeSeq::end(self)
    }
}
impl SerializeTupleStruct for Sequence {
    type Ok = Value;
    type Error = serde_json::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Self::Error> {
        SerializeSeq::serialize_element(self, v)
    }
    fn end(self) -> Result<Value, Self::Error> {
        SerializeSeq::end(self)
    }
}
impl SerializeTupleVariant for Sequence {
    type Ok = Value;
    type Error = serde_json::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Self::Error> {
        SerializeSeq::serialize_element(self, v)
    }
    fn end(self) -> Result<Value, Self::Error> {
        SerializeSeq::end(self)
    }
}
impl SerializeStruct for Object {
    type Ok = Value;
    type Error = serde_json::Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        v: &T,
    ) -> Result<(), Self::Error> {
        self.fields.insert(key.into(), to_value(v)?);
        Ok(())
    }
    fn end(mut self) -> Result<Value, Self::Error> {
        let name = self
            .fields
            .get("_type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        for (key, value) in &mut self.fields {
            let label = |v: &mut Value| {
                if let Some(number) = v.as_i64().and_then(|v| i32::try_from(v).ok()) {
                    if let Some(label) =
                        tricerules_proto::diagnostic_enums::name(&name, key, number)
                    {
                        *v = Value::String(label.to_owned());
                    }
                }
            };
            if let Some(values) = value.as_array_mut() {
                for value in values {
                    label(value);
                }
            } else {
                label(value);
            }
        }
        Ok(variant(self.variant, Value::Object(self.fields)))
    }
}
impl SerializeStructVariant for Object {
    type Ok = Value;
    type Error = serde_json::Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        v: &T,
    ) -> Result<(), Self::Error> {
        SerializeStruct::serialize_field(self, key, v)
    }
    fn end(self) -> Result<Value, Self::Error> {
        SerializeStruct::end(self)
    }
}
impl SerializeMap for Mapping {
    type Ok = Value;
    type Error = serde_json::Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Self::Error> {
        self.key = Some(to_value(v)?);
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), Self::Error> {
        let key = self
            .key
            .take()
            .ok_or_else(|| ser::Error::custom("map value without key"))?;
        self.entries.push((key, to_value(v)?));
        Ok(())
    }
    fn end(mut self) -> Result<Value, Self::Error> {
        self.entries.sort_by_cached_key(|(key, _)| key.to_string());
        if self
            .entries
            .iter()
            .all(|(key, _)| key.is_string() || key.is_number() || key.is_boolean())
        {
            Ok(Value::Object(
                self.entries
                    .into_iter()
                    .map(|(k, v)| {
                        (
                            k.as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| k.to_string()),
                            v,
                        )
                    })
                    .collect(),
            ))
        } else {
            Ok(
                serde_json::json!({"entries": self.entries.into_iter().map(|(key, value)|
                serde_json::json!({"key": key, "value": value})).collect::<Vec<_>>()}),
            )
        }
    }
}
