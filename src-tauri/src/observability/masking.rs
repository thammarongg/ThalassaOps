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
        }
    }
    masked
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
}
