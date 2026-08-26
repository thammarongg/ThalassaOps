pub const REDACTED: &str = "<REDACTED>";

pub fn sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["password", "secret", "token", "key", "credential"]
        .iter()
        .any(|needle| key.contains(needle))
}

pub fn mask_json_object(object: &mut serde_json::Map<String, serde_json::Value>) -> bool {
    let mut masked = false;
    for (key, value) in object.iter_mut() {
        if sensitive_key(key) {
            *value = serde_json::Value::String(REDACTED.into());
            masked = true;
        } else if mask_json_value(value) {
            masked = true;
        }
    }
    masked
}

fn mask_json_value(value: &mut serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => mask_json_object(object),
        serde_json::Value::Array(values) => {
            let mut masked = false;
            for value in values {
                if mask_json_value(value) {
                    masked = true;
                }
            }
            masked
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_key_matches_the_sprint_7_semantics() {
        for key in [
            "password",
            "API_KEY",
            "client_secret",
            "authToken",
            "credential",
        ] {
            assert!(sensitive_key(key), "{key}");
        }
        for key in ["namespace", "message", "level", "duration_ms"] {
            assert!(!sensitive_key(key), "{key}");
        }
    }

    #[test]
    fn mask_json_object_replaces_only_sensitive_values() {
        let mut object = serde_json::json!({ "msg": "hello", "api_key": "sk-live-1" })
            .as_object()
            .unwrap()
            .clone();
        assert!(mask_json_object(&mut object));
        assert_eq!(object["msg"], serde_json::json!("hello"));
        assert_eq!(object["api_key"], serde_json::json!(REDACTED));
    }

    #[test]
    fn mask_json_object_recurses_through_nested_objects_and_arrays() {
        let mut object = serde_json::json!({
            "context": {"client_secret": "nested-secret"},
            "items": [
                {"password": "array-secret"},
                {"nested": {"access_token": "deep-token"}}
            ]
        })
        .as_object()
        .unwrap()
        .clone();

        assert!(mask_json_object(&mut object));
        assert_eq!(
            object["context"]["client_secret"],
            serde_json::json!(REDACTED)
        );
        assert_eq!(object["items"][0]["password"], serde_json::json!(REDACTED));
        assert_eq!(
            object["items"][1]["nested"]["access_token"],
            serde_json::json!(REDACTED)
        );
    }
}
