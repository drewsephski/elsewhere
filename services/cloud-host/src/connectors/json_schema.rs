use serde_json::{Map, Value};

use agent_core::ConnectorError;

const MAX_SCHEMA_DEPTH: usize = 8;
const MAX_OBJECT_KEYS: usize = 64;
const MAX_ARRAY_ITEMS: usize = 64;

pub fn validate_args_against_schema(schema: &Value, args: &Value) -> Result<(), ConnectorError> {
    validate_value(schema, args, 0)
}

fn validate_value(schema: &Value, value: &Value, depth: usize) -> Result<(), ConnectorError> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(ConnectorError::Validation(
            "argument schema is too deep".into(),
        ));
    }
    let Some(obj) = schema.as_object() else {
        return Ok(());
    };
    if let Some(types) = schema_types(obj) {
        if !types.iter().any(|t| type_matches(t, value)) {
            return Err(ConnectorError::Validation(format!(
                "argument does not match schema type {}",
                types.join("|")
            )));
        }
    }
    if let Some(enum_values) = obj.get("enum").and_then(|v| v.as_array()) {
        if !enum_values.iter().any(|allowed| allowed == value) {
            return Err(ConnectorError::Validation(
                "argument is not one of the allowed values".into(),
            ));
        }
    }
    match value {
        Value::Object(map) => validate_object(obj, map, depth),
        Value::Array(items) => validate_array(obj, items, depth),
        _ => Ok(()),
    }
}

fn validate_object(
    schema: &Map<String, Value>,
    value: &Map<String, Value>,
    depth: usize,
) -> Result<(), ConnectorError> {
    if value.len() > MAX_OBJECT_KEYS {
        return Err(ConnectorError::Validation(
            "too many argument fields".into(),
        ));
    }
    if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
        for key in required {
            let Some(name) = key.as_str() else {
                continue;
            };
            if !value.contains_key(name) {
                return Err(ConnectorError::Validation(format!(
                    "missing required argument `{name}`"
                )));
            }
        }
    }
    let additional = schema
        .get("additionalProperties")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let properties = schema
        .get("properties")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    for (key, child) in value {
        match properties.get(key) {
            Some(child_schema) => validate_value(child_schema, child, depth + 1)?,
            None if additional => {}
            None => {
                return Err(ConnectorError::Validation(format!(
                    "unexpected argument `{key}`"
                )));
            }
        }
    }
    Ok(())
}

fn validate_array(
    schema: &Map<String, Value>,
    items: &[Value],
    depth: usize,
) -> Result<(), ConnectorError> {
    if items.len() > MAX_ARRAY_ITEMS {
        return Err(ConnectorError::Validation("too many array items".into()));
    }
    if let Some(item_schema) = schema.get("items") {
        for item in items {
            validate_value(item_schema, item, depth + 1)?;
        }
    }
    Ok(())
}

fn schema_types(schema: &Map<String, Value>) -> Option<Vec<String>> {
    match schema.get("type") {
        Some(Value::String(t)) => Some(vec![t.clone()]),
        Some(Value::Array(items)) => Some(
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
        ),
        _ => None,
    }
}

fn type_matches(expected: &str, value: &Value) -> bool {
    match expected {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_missing_required() {
        let schema = json!({
            "type": "object",
            "properties": { "q": { "type": "string" } },
            "required": ["q"],
            "additionalProperties": false
        });
        let err = validate_args_against_schema(&schema, &json!({})).unwrap_err();
        assert!(matches!(err, ConnectorError::Validation(_)));
    }

    #[test]
    fn rejects_additional_properties() {
        let schema = json!({
            "type": "object",
            "properties": { "q": { "type": "string" } },
            "additionalProperties": false
        });
        let err = validate_args_against_schema(&schema, &json!({ "q": "hi", "extra": true }))
            .unwrap_err();
        assert!(matches!(err, ConnectorError::Validation(_)));
    }

    #[test]
    fn accepts_valid_object() {
        let schema = json!({
            "type": "object",
            "properties": { "q": { "type": "string" } },
            "required": ["q"],
            "additionalProperties": false
        });
        validate_args_against_schema(&schema, &json!({ "q": "hello" })).unwrap();
    }
}
