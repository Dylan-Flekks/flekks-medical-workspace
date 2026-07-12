use std::collections::BTreeSet;
use std::fmt;

use serde::de::DeserializeSeed;
use serde::de::Error as _;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;

use super::MAX_ISOLATED_OUTPUT_SCHEMA_BYTES;
use super::MAX_ISOLATED_OUTPUT_SCHEMA_DEPTH;

pub(super) fn validate_strict_object_schema(schema: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(schema)
        .map_err(|error| format!("isolated output schema is invalid: {error}"))?;
    if bytes.len() > MAX_ISOLATED_OUTPUT_SCHEMA_BYTES {
        return Err(format!(
            "isolated output schema exceeds the {MAX_ISOLATED_OUTPUT_SCHEMA_BYTES}-byte limit"
        ));
    }
    let root = schema
        .as_object()
        .ok_or_else(|| "isolated output schema must be an object".to_string())?;
    if root.get("type").and_then(Value::as_str) != Some("object") {
        return Err("isolated output schema root type must be object".to_string());
    }
    validate_schema_node(schema, 0)
}

pub(super) fn validate_value(schema: &Value, value: &Value) -> Result<(), String> {
    validate_value_at(schema, value, "$")
}

pub(super) fn parse_unique_json(text: &str) -> Result<Value, String> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = UniqueValueSeed
        .deserialize(&mut deserializer)
        .map_err(|error| error.to_string())?;
    deserializer.end().map_err(|error| error.to_string())?;
    Ok(value)
}

struct UniqueValueSeed;

impl<'de> DeserializeSeed<'de> for UniqueValueSeed {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueValueVisitor)
    }
}

struct UniqueValueVisitor;

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_string(value.to_string())
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(UniqueValueSeed)? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(A::Error::custom(format!(
                    "duplicate JSON object key `{key}`"
                )));
            }
            values.insert(key, object.next_value_seed(UniqueValueSeed)?);
        }
        Ok(Value::Object(values))
    }
}

fn validate_schema_node(schema: &Value, depth: usize) -> Result<(), String> {
    if depth > MAX_ISOLATED_OUTPUT_SCHEMA_DEPTH {
        return Err("isolated output schema exceeds the nesting limit".to_string());
    }
    let node = schema
        .as_object()
        .ok_or_else(|| "every isolated output schema node must be an object".to_string())?;
    let kind = node
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "every isolated output schema node must have one string type".to_string())?;
    let allowed = match kind {
        "object" => &[
            "type",
            "description",
            "title",
            "properties",
            "required",
            "additionalProperties",
        ][..],
        "array" => &["type", "description", "title", "items"][..],
        "string" | "number" | "integer" | "boolean" | "null" => {
            &["type", "description", "title", "enum", "const"][..]
        }
        _ => return Err(format!("unsupported isolated output schema type `{kind}`")),
    };
    if let Some(key) = node.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!(
            "unsupported isolated output schema keyword `{key}`"
        ));
    }
    match kind {
        "object" => validate_object_schema(node, depth),
        "array" => {
            let items = node
                .get("items")
                .ok_or_else(|| "every isolated array schema must define items".to_string())?;
            validate_schema_node(items, depth + 1)
        }
        _ => validate_scalar_literals(node, kind),
    }
}

fn validate_object_schema(node: &Map<String, Value>, depth: usize) -> Result<(), String> {
    let properties = node
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| "every isolated object schema must define properties".to_string())?;
    if node.get("additionalProperties") != Some(&Value::Bool(false)) {
        return Err(
            "every isolated object schema must set additionalProperties to false".to_string(),
        );
    }
    let required = node
        .get("required")
        .and_then(Value::as_array)
        .ok_or_else(|| "every isolated object schema must define required".to_string())?;
    let mut required_names = BTreeSet::new();
    for value in required {
        let name = value
            .as_str()
            .ok_or_else(|| "isolated output schema required entries must be strings".to_string())?;
        if !required_names.insert(name) {
            return Err("isolated output schema required entries must be unique".to_string());
        }
    }
    let property_names = properties
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if required_names != property_names {
        return Err(
            "every isolated object schema must require exactly all defined properties".to_string(),
        );
    }
    for child in properties.values() {
        validate_schema_node(child, depth + 1)?;
    }
    Ok(())
}

fn validate_scalar_literals(node: &Map<String, Value>, kind: &str) -> Result<(), String> {
    if let Some(values) = node.get("enum") {
        let values = values
            .as_array()
            .filter(|values| !values.is_empty())
            .ok_or_else(|| "isolated output schema enum must be a non-empty array".to_string())?;
        for value in values {
            validate_primitive_type(kind, value, "schema enum")?;
        }
    }
    if let Some(value) = node.get("const") {
        validate_primitive_type(kind, value, "schema const")?;
    }
    Ok(())
}

fn validate_value_at(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    let node = schema
        .as_object()
        .ok_or_else(|| "isolated output schema changed after validation".to_string())?;
    let kind = node
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "isolated output schema changed after validation".to_string())?;
    match kind {
        "object" => {
            let output = value
                .as_object()
                .ok_or_else(|| wrong_type(path, "object"))?;
            let properties = node["properties"]
                .as_object()
                .ok_or_else(|| "isolated output schema changed after validation".to_string())?;
            if let Some(name) = output.keys().find(|name| !properties.contains_key(*name)) {
                return Err(format!(
                    "isolated assistant output has unknown field `{path}.{name}`"
                ));
            }
            for (name, child_schema) in properties {
                let child = output.get(name).ok_or_else(|| {
                    format!("isolated assistant output is missing `{path}.{name}`")
                })?;
                validate_value_at(child_schema, child, &format!("{path}.{name}"))?;
            }
        }
        "array" => {
            let output = value.as_array().ok_or_else(|| wrong_type(path, "array"))?;
            let items = &node["items"];
            for (index, child) in output.iter().enumerate() {
                validate_value_at(items, child, &format!("{path}[{index}]"))?;
            }
        }
        _ => validate_primitive_type(kind, value, path)?,
    }
    if let Some(values) = node.get("enum").and_then(Value::as_array)
        && !values.contains(value)
    {
        return Err(format!(
            "isolated assistant output value at `{path}` is not allowed"
        ));
    }
    if let Some(expected) = node.get("const")
        && expected != value
    {
        return Err(format!(
            "isolated assistant output value at `{path}` does not match const"
        ));
    }
    Ok(())
}

fn validate_primitive_type(kind: &str, value: &Value, path: &str) -> Result<(), String> {
    let valid = match kind {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value
            .as_number()
            .is_some_and(|number| number.is_i64() || number.is_u64()),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(wrong_type(path, kind))
    }
}

fn wrong_type(path: &str, expected: &str) -> String {
    format!("isolated assistant output at `{path}` must be {expected}")
}
