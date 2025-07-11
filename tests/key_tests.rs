use autonomi::SecretKey;
use colonylib::KeyStore;

#[test]
fn test_key_store_from_mnemonic() {
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mut key_store = KeyStore::from_mnemonic(mnemonic).unwrap();

    assert_eq!(key_store.get_seed_phrase(), mnemonic);

    // Initially, one configuration pointer key should exist
    assert_eq!(key_store.get_pointers().len(), 1);

    // Add another pointer key and verify it exists
    key_store.add_pointer_key().unwrap();
    assert_eq!(key_store.get_pointers().len(), 2);
}

#[test]
fn test_key_store_to_and_from_file() {
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let key_store = KeyStore::from_mnemonic(mnemonic).unwrap();

    let password = "test_password";
    let mut file = std::io::Cursor::new(Vec::new());

    key_store.to_file(&mut file, password).unwrap();
    file.set_position(0);

    let loaded_key_store = KeyStore::from_file(&mut file, password).unwrap();

    assert_eq!(
        key_store.get_seed_phrase(),
        loaded_key_store.get_seed_phrase()
    );
    assert_eq!(
        key_store.get_pointers().len(),
        loaded_key_store.get_pointers().len()
    );
}

#[test]
fn test_wallet_key_operations() {
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mut key_store = KeyStore::from_mnemonic(mnemonic).unwrap();

    // Initially, no wallet keys should exist
    assert!(key_store.get_wallet_key("main").is_err());
    assert!(key_store.get_wallet_key("backup").is_err());

    // Generate valid wallet keys using SecretKey::random()
    let main_secret_key = SecretKey::random();
    let main_key = hex::encode(main_secret_key.to_bytes());
    key_store.add_wallet_key("main", &main_key).unwrap();

    // Generate another valid wallet key
    let backup_secret_key = SecretKey::random();
    let backup_key = hex::encode(backup_secret_key.to_bytes());
    key_store.add_wallet_key("backup", &backup_key).unwrap();

    // Retrieve the keys and verify they match
    let retrieved_main = key_store.get_wallet_key("main").unwrap();
    assert_eq!(retrieved_main, main_key);

    let retrieved_backup = key_store.get_wallet_key("backup").unwrap();
    assert_eq!(retrieved_backup, backup_key);

    // Try to get a non-existent key
    assert!(key_store.get_wallet_key("nonexistent").is_err());

    // Overwrite an existing key with another valid key
    let new_main_secret_key = SecretKey::random();
    let new_main_key = hex::encode(new_main_secret_key.to_bytes());
    key_store.add_wallet_key("main", &new_main_key).unwrap();
    let retrieved_new_main = key_store.get_wallet_key("main").unwrap();
    assert_eq!(retrieved_new_main, new_main_key);
}

#[test]
fn test_wallet_key_persistence() {
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mut key_store = KeyStore::from_mnemonic(mnemonic).unwrap();

    // Add wallet keys using valid SecretKeys
    let main_secret_key = SecretKey::random();
    let main_key = hex::encode(main_secret_key.to_bytes());
    key_store.add_wallet_key("main", &main_key).unwrap();

    let backup_secret_key = SecretKey::random();
    let backup_key = hex::encode(backup_secret_key.to_bytes());
    key_store.add_wallet_key("backup", &backup_key).unwrap();

    // Save to file
    let password = "test_password";
    let mut file = std::io::Cursor::new(Vec::new());
    key_store.to_file(&mut file, password).unwrap();
    file.set_position(0);

    // Load from file
    let loaded_key_store = KeyStore::from_file(&mut file, password).unwrap();

    // Verify wallet keys are preserved
    assert_eq!(loaded_key_store.get_wallet_key("main").unwrap(), main_key);
    assert_eq!(
        loaded_key_store.get_wallet_key("backup").unwrap(),
        backup_key
    );
}

#[test]
fn test_get_wallet_addresses_comprehensive() {
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mut key_store = KeyStore::from_mnemonic(mnemonic).unwrap();

    // Test with multiple valid keys
    let key1 = SecretKey::random();
    let key1_hex = hex::encode(key1.to_bytes());
    key_store.add_wallet_key("key1", &key1_hex).unwrap();

    let key2 = SecretKey::random();
    let key2_hex = hex::encode(key2.to_bytes());
    key_store.add_wallet_key("key2", &key2_hex).unwrap();

    let key3 = SecretKey::random();
    let key3_hex = hex::encode(key3.to_bytes());
    key_store.add_wallet_key("key3", &key3_hex).unwrap();

    // Get wallet addresses - this should not panic
    let addresses = key_store.get_wallet_addresses();

    // Should have addresses for all keys
    assert!(addresses.contains_key("key1"));
    assert!(addresses.contains_key("key2"));
    assert!(addresses.contains_key("key3"));

    // Each key should produce the expected address
    let expected_address1 = key1.public_key().to_hex();
    let expected_address2 = key2.public_key().to_hex();
    let expected_address3 = key3.public_key().to_hex();

    assert_eq!(addresses.get("key1").unwrap(), &expected_address1);
    assert_eq!(addresses.get("key2").unwrap(), &expected_address2);
    assert_eq!(addresses.get("key3").unwrap(), &expected_address3);

    // All addresses should be valid (96 characters for PublicKey hex)
    assert_eq!(addresses.get("key1").unwrap().len(), 96);
    assert_eq!(addresses.get("key2").unwrap().len(), 96);
    assert_eq!(addresses.get("key3").unwrap().len(), 96);

    // All addresses should be different
    assert_ne!(
        addresses.get("key1").unwrap(),
        addresses.get("key2").unwrap()
    );
    assert_ne!(
        addresses.get("key2").unwrap(),
        addresses.get("key3").unwrap()
    );
    assert_ne!(
        addresses.get("key1").unwrap(),
        addresses.get("key3").unwrap()
    );
}

#[test]
fn test_add_wallet_key_validation() {
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mut key_store = KeyStore::from_mnemonic(mnemonic).unwrap();

    // Test that invalid keys are rejected
    let invalid_key = "invalid_key";
    assert!(key_store.add_wallet_key("invalid", invalid_key).is_err());

    let short_key = "1234";
    assert!(key_store.add_wallet_key("short", short_key).is_err());

    let long_key = "123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678";
    assert!(key_store.add_wallet_key("long", long_key).is_err());

    // Test that valid keys are accepted
    let valid_key = SecretKey::random();
    let valid_key_hex = hex::encode(valid_key.to_bytes());
    assert!(key_store.add_wallet_key("valid", &valid_key_hex).is_ok());
}

#[test]
fn test_get_wallet_addresses_with_random_valid_key() {
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mut key_store = KeyStore::from_mnemonic(mnemonic).unwrap();

    // Generate a random valid private key
    let random_key = SecretKey::random();
    let random_key_hex = hex::encode(random_key.to_bytes());

    println!("Generated random private key: {}", random_key_hex);

    // Add the key to the store
    key_store
        .add_wallet_key("random_test", &random_key_hex)
        .unwrap();

    // Get wallet addresses - this should work without panicking
    let addresses = key_store.get_wallet_addresses();

    // Verify the address was generated correctly
    assert!(addresses.contains_key("random_test"));
    let generated_address = addresses.get("random_test").unwrap();
    let expected_address = random_key.public_key().to_hex();

    println!("Generated address: {}", generated_address);
    println!("Expected address:  {}", expected_address);

    assert_eq!(generated_address, &expected_address);
    assert_eq!(generated_address.len(), 96); // PublicKey hex should be 96 characters
}
