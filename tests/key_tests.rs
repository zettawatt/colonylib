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
