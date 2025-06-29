mod common;
use common::create_test_datastore;

#[test]
fn test_datastore_creation() {
    let (datastore, _temp_dir) = create_test_datastore();

    // Check that directories were created
    assert!(datastore.get_pods_dir().exists());
    assert!(datastore.get_pointers_dir().exists());
    assert!(datastore.get_scratchpads_dir().exists());
    assert!(datastore.get_downloads_path().exists());
}

#[test]
fn test_pointer_operations() {
    let (datastore, _temp_dir) = create_test_datastore();

    let address = "test_pointer_address";
    let target = "test_target_address";

    // Create pointer file
    datastore.create_pointer_file(address).unwrap();
    assert!(datastore.address_is_pointer(address).unwrap());

    // Update pointer target
    datastore.update_pointer_target(address, target).unwrap();
    let retrieved_target = datastore.get_pointer_target(address).unwrap();
    assert_eq!(retrieved_target, target);

    // Update pointer count
    datastore.update_pointer_count(address, 5).unwrap();
    let count = datastore.get_pointer_count(address).unwrap();
    assert_eq!(count, 5);
}

#[test]
fn test_scratchpad_operations() {
    let (datastore, _temp_dir) = create_test_datastore();

    let address = "test_scratchpad_address";
    let data = "test scratchpad data content";

    // Create scratchpad file
    datastore.create_scratchpad_file(address).unwrap();
    assert!(datastore.address_is_scratchpad(address).unwrap());

    // Update scratchpad data
    datastore.update_scratchpad_data(address, data).unwrap();
    let retrieved_data = datastore.get_scratchpad_data(address).unwrap();
    assert_eq!(retrieved_data, data);
}

#[test]
fn test_update_list_operations() {
    let (datastore, _temp_dir) = create_test_datastore();

    let address1 = "address1";
    let address2 = "address2";

    // Append to update list
    datastore.append_update_list(address1).unwrap();
    datastore.append_update_list(address2).unwrap();

    // Check that update list file exists and is in JSON format
    let update_list_path = datastore.get_update_list_path();
    assert!(update_list_path.exists());
    assert!(update_list_path.extension().unwrap() == "json");

    // Check that the addresses are in the pods section
    let update_list = datastore.get_update_list().unwrap();
    assert!(update_list.pods.contains_key(address1));
    assert!(update_list.pods.contains_key(address2));
}

#[test]
fn test_address_validation() {
    let (datastore, _temp_dir) = create_test_datastore();

    let non_existent_address = "non_existent_address";

    // Should return false for non-existent addresses
    assert!(!datastore.address_is_pointer(non_existent_address).unwrap());
    assert!(
        !datastore
            .address_is_scratchpad(non_existent_address)
            .unwrap()
    );
}

#[test]
fn test_path_getters() {
    let (datastore, temp_dir) = create_test_datastore();

    // Test all path getters
    assert!(datastore.get_pods_dir().starts_with(temp_dir.path()));
    assert!(datastore.get_pointers_dir().starts_with(temp_dir.path()));
    assert!(datastore.get_scratchpads_dir().starts_with(temp_dir.path()));
    assert!(datastore.get_downloads_path().starts_with(temp_dir.path()));
    assert!(datastore.get_data_path().starts_with(temp_dir.path()));
    assert!(datastore.get_keystore_path().starts_with(temp_dir.path()));
    assert!(datastore.get_graph_path().starts_with(temp_dir.path()));
    assert!(
        datastore
            .get_update_list_path()
            .starts_with(temp_dir.path())
    );
}

#[test]
fn test_error_handling() {
    let (datastore, _temp_dir) = create_test_datastore();

    let non_existent_address = "non_existent_address";

    // These should return errors for non-existent files
    assert!(datastore.get_pointer_target(non_existent_address).is_err());
    assert!(datastore.get_pointer_count(non_existent_address).is_err());
    assert!(datastore.get_scratchpad_data(non_existent_address).is_err());
}

#[test]
fn test_duplicate_update_list_entries() {
    let (datastore, _temp_dir) = create_test_datastore();

    let address = "duplicate_test_address";

    // Add the same address multiple times
    datastore.append_update_list(address).unwrap();
    datastore.append_update_list(address).unwrap();
    datastore.append_update_list(address).unwrap();

    // Check that it only appears once in the pods section
    let update_list = datastore.get_update_list().unwrap();
    assert_eq!(update_list.pods.len(), 1);
    assert!(update_list.pods.contains_key(address));
}
