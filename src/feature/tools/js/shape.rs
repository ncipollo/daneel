use serde_json::Value;

/// Checks `value` against a small JSON-schema subset: `type`, `properties`,
/// `required` and `items`. The error says where the mismatch is.
pub fn check_shape(value: &Value, shape: &Value) -> Result<(), String> {
    check(value, shape, "result")
}

fn check(value: &Value, shape: &Value, path: &str) -> Result<(), String> {
    if let Some(expected) = shape.get("type").and_then(Value::as_str)
        && !is_type(value, expected)
    {
        return Err(format!(
            "{path} should be {expected} but is {}",
            kind(value)
        ));
    }
    match value {
        Value::Object(map) => check_object(map, shape, path),
        Value::Array(items) => check_items(items, shape, path),
        _ => Ok(()),
    }
}

fn check_object(
    map: &serde_json::Map<String, Value>,
    shape: &Value,
    path: &str,
) -> Result<(), String> {
    let required = shape.get("required").and_then(Value::as_array);
    for name in required.into_iter().flatten().filter_map(Value::as_str) {
        if !map.contains_key(name) {
            return Err(format!("{path}.{name} is missing"));
        }
    }
    let properties = shape.get("properties").and_then(Value::as_object);
    for (name, sub_shape) in properties.into_iter().flatten() {
        if let Some(value) = map.get(name) {
            check(value, sub_shape, &format!("{path}.{name}"))?;
        }
    }
    Ok(())
}

fn check_items(items: &[Value], shape: &Value, path: &str) -> Result<(), String> {
    let Some(item_shape) = shape.get("items") else {
        return Ok(());
    };
    items
        .iter()
        .enumerate()
        .try_for_each(|(i, item)| check(item, item_shape, &format!("{path}[{i}]")))
}

fn is_type(value: &Value, expected: &str) -> bool {
    match expected {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_f64().is_some_and(|n| n.fract() == 0.0),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "null" => value.is_null(),
        _ => true,
    }
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn matching_values_pass() {
        let shape = json!({
            "type": "object",
            "required": ["n"],
            "properties": { "n": { "type": "integer" }, "tags": { "type": "array", "items": { "type": "string" } } }
        });
        assert!(check_shape(&json!({ "n": 3.0, "tags": ["a"] }), &shape).is_ok());
    }

    #[test]
    fn the_wrong_type_names_the_path() {
        let shape = json!({ "type": "object", "properties": { "n": { "type": "number" } } });
        let err = check_shape(&json!({ "n": "x" }), &shape).unwrap_err();
        assert_eq!(err, "result.n should be number but is string");
    }

    #[test]
    fn missing_required_fields_and_bad_items_are_caught() {
        let shape = json!({ "type": "object", "required": ["a"] });
        assert_eq!(
            check_shape(&json!({}), &shape).unwrap_err(),
            "result.a is missing"
        );
        let shape = json!({ "type": "array", "items": { "type": "string" } });
        let err = check_shape(&json!(["a", 2]), &shape).unwrap_err();
        assert_eq!(err, "result[1] should be string but is number");
    }

    #[test]
    fn fractions_are_not_integers_and_empty_shapes_accept_anything() {
        assert!(check_shape(&json!(1.5), &json!({ "type": "integer" })).is_err());
        assert!(check_shape(&json!([1, "x"]), &json!({})).is_ok());
    }
}
