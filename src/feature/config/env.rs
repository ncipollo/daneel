use serde_json::Value;

/// Replaces every string starting with `$` by the value `lookup` returns for the
/// rest of the string. Fails with the name of the first unset variable.
pub fn resolve_tree(
    value: Value,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<Value, String> {
    match value {
        Value::String(s) => resolve_string(s, lookup).map(Value::String),
        Value::Array(items) => items
            .into_iter()
            .map(|item| resolve_tree(item, lookup))
            .collect::<Result<_, _>>()
            .map(Value::Array),
        Value::Object(map) => map
            .into_iter()
            .map(|(key, item)| resolve_tree(item, lookup).map(|v| (key, v)))
            .collect::<Result<_, _>>()
            .map(Value::Object),
        other => Ok(other),
    }
}

fn resolve_string(s: String, lookup: &dyn Fn(&str) -> Option<String>) -> Result<String, String> {
    match s.strip_prefix('$') {
        Some(var) => lookup(var).ok_or_else(|| var.to_string()),
        None => Ok(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lookup(name: &str) -> Option<String> {
        (name == "SET").then(|| "expanded".to_string())
    }

    #[test]
    fn literal_passes_through() {
        let value = json!({ "url": "http://localhost:11434", "n": 3 });
        assert_eq!(resolve_tree(value.clone(), &lookup), Ok(value));
    }

    #[test]
    fn set_variable_is_resolved() {
        assert_eq!(resolve_tree(json!("$SET"), &lookup), Ok(json!("expanded")));
    }

    #[test]
    fn nested_values_are_resolved() {
        let value = json!({ "providers": { "a": { "models": ["$SET", "plain"] } } });
        let expected = json!({ "providers": { "a": { "models": ["expanded", "plain"] } } });
        assert_eq!(resolve_tree(value, &lookup), Ok(expected));
    }

    #[test]
    fn unset_variable_reports_its_name() {
        assert_eq!(
            resolve_tree(json!({ "k": "$MISSING" }), &lookup),
            Err("MISSING".to_string())
        );
    }
}
