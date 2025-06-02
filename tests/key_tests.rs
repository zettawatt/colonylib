use colonylib::KeyStore;

#[test]
fn test_key_store_from_mnemonic() {
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mut key_store = KeyStore::from_mnemonic(mnemonic).unwrap();

    assert_eq!(key_store.get_seed_phrase(), mnemonic);

    // Initially, no pointers should exist
    assert_eq!(key_store.get_pointers().len(), 0);

    // Add a pointer key and verify it exists
    key_store.add_pointer_key().unwrap();
    assert!(key_store.get_pointers().len() > 0);
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

    assert_eq!(key_store.get_seed_phrase(), loaded_key_store.get_seed_phrase());
    assert_eq!(key_store.get_pointers().len(), loaded_key_store.get_pointers().len());
}
