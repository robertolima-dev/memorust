use memors::store::Store;

#[test]
fn should_create_empty_store() {
    let store = Store::new();

    assert_eq!(store.len(), 0);
    assert!(store.is_empty());
}

#[test]
fn should_return_none_when_key_does_not_exist() {
    let mut store = Store::new();

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
