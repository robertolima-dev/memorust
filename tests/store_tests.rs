use memorust::store::Store;

#[test]
fn should_create_empty_store() {
    let store = Store::new();

    assert_eq!(store.len(), 0);
    assert!(store.is_empty());
}

#[test]
fn should_return_none_when_key_does_not_exist() {
    let store = Store::new();

    assert_eq!(store.get("unknown"), None);
}

#[test]
fn should_delete_existing_key() {
    let mut store = Store::new();

    store.set("name".to_string(), "Roberto".to_string());

    let deleted = store.del("name");

    assert!(deleted);
    assert_eq!(store.get("name"), None);
}

#[test]
fn should_return_false_when_deleting_unknown_key() {
    let mut store = Store::new();

    let deleted = store.del("unknown");

    assert!(!deleted);
}

#[test]
fn should_check_if_key_exists() {
    let mut store = Store::new();

    store.set("name".to_string(), "Roberto".to_string());

    assert!(store.exists("name"));
    assert!(!store.exists("unknown"));
}

#[test]
fn expire_at_should_expire_key_using_absolute_timestamp() {
    let mut store = Store::new();

    store.set("token".to_string(), "abc".to_string());

    let expires_at = Store::expiration_unix_ms_from_seconds(1);

    let result = store.expire_at("token", expires_at);

    assert!(result);
    assert_eq!(store.get("token"), Some(&"abc".to_string()));

    std::thread::sleep(std::time::Duration::from_secs(2));

    assert_eq!(store.get("token"), None);
}
