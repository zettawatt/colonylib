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

    // Add a main wallet key
    let main_key = "0x1234512345123451234512345123451234512345123451234512345123451234";
    key_store.add_wallet_key("main", main_key).unwrap();

    // Add a backup wallet key with 0x prefix
    let backup_key = "0xabcdabcde12345abcde12345abcde12345abcde12345abcde12345eabcde1234";
    key_store.add_wallet_key("backup", backup_key).unwrap();

    // Retrieve the keys and verify they match (without 0x prefix)
    let retrieved_main = key_store.get_wallet_key("main").unwrap();
    assert_eq!(
        retrieved_main,
        "1234512345123451234512345123451234512345123451234512345123451234"
    );

    let retrieved_backup = key_store.get_wallet_key("backup").unwrap();
    assert_eq!(
        retrieved_backup,
        "abcdabcde12345abcde12345abcde12345abcde12345abcde12345eabcde1234"
    );

    // Try to get a non-existent key
    assert!(key_store.get_wallet_key("nonexistent").is_err());

    // Overwrite an existing key
    let new_main_key = "1234567890123456789012345678901234567890123456789012345678901234";
    key_store.add_wallet_key("main", new_main_key).unwrap();
    let retrieved_new_main = key_store.get_wallet_key("main").unwrap();
    assert_eq!(retrieved_new_main, new_main_key);
}

#[test]
fn test_wallet_key_persistence() {
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mut key_store = KeyStore::from_mnemonic(mnemonic).unwrap();

    // Add wallet keys
    key_store
        .add_wallet_key(
            "main",
            "0x1234512345123451234512345123451234512345123451234512345123451234",
        )
        .unwrap();
    key_store
        .add_wallet_key(
            "backup",
            "0xabcdabcde12345abcde12345abcde12345abcde12345abcde12345eabcde1234",
        )
        .unwrap();

    // Save to file
    let password = "test_password";
    let mut file = std::io::Cursor::new(Vec::new());
    key_store.to_file(&mut file, password).unwrap();
    file.set_position(0);

    // Load from file
    let loaded_key_store = KeyStore::from_file(&mut file, password).unwrap();

    // Verify wallet keys are preserved
    assert_eq!(
        loaded_key_store.get_wallet_key("main").unwrap(),
        "1234512345123451234512345123451234512345123451234512345123451234"
    );
    assert_eq!(
        loaded_key_store.get_wallet_key("backup").unwrap(),
        "abcdabcde12345abcde12345abcde12345abcde12345abcde12345eabcde1234"
    );
}

#[test]
fn test_get_wallet_addresses_comprehensive() {
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mut key_store = KeyStore::from_mnemonic(mnemonic).unwrap();

    // Test with a valid private key (32 bytes = 64 hex characters)
    let valid_key = "1234567890123456789012345678901234567890123456789012345678901234";
    key_store.add_wallet_key("valid", valid_key).unwrap();

    // Test with an invalid key (wrong length for SecretKey but valid hex)
    let short_key = "1234"; // Valid hex but too short for SecretKey
    key_store.add_wallet_key("short", short_key).unwrap();

    // Test with an invalid key (wrong length - too long)
    let long_key = "123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678";
    key_store.add_wallet_key("long", long_key).unwrap();

    // Test with a randomly generated valid key
    let random_key = SecretKey::random();
    let random_key_hex = hex::encode(random_key.to_bytes());
    key_store.add_wallet_key("random", &random_key_hex).unwrap();

    // Get wallet addresses - this should not panic
    let addresses = key_store.get_wallet_addresses();

    // Should have addresses for all keys
    assert!(addresses.contains_key("valid"));
    assert!(addresses.contains_key("short"));
    assert!(addresses.contains_key("long"));
    assert!(addresses.contains_key("random"));

    // The random key should produce the expected address
    let expected_address = random_key.public_key().to_hex();
    assert_eq!(addresses.get("random").unwrap(), &expected_address);

    // Invalid keys should have the default address
    let default_address = "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
    assert_eq!(addresses.get("short").unwrap(), default_address);
    assert_eq!(addresses.get("long").unwrap(), default_address);

    // Valid key should have a proper address (not the default)
    let valid_address = addresses.get("valid").unwrap();
    assert_ne!(valid_address, default_address);
    assert_eq!(valid_address.len(), 96); // PublicKey hex should be 96 characters (48 bytes)
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
