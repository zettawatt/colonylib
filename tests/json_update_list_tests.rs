use std::fs;

mod common;
use colonylib::pod::UpdateList;
use common::create_test_datastore;

#[test]
fn test_json_update_list_creation() {
    let (datastore, _temp_dir) = create_test_datastore();

    // Initially, the update list should be empty
    let update_list = datastore.get_update_list().unwrap();
    assert!(update_list.pods.is_empty());
    assert!(update_list.remove.pointers.is_empty());
    assert!(update_list.remove.scratchpads.is_empty());
}

#[test]
fn test_append_pod_to_update_list() {
    let (datastore, _temp_dir) = create_test_datastore();

    let pod_address = "test_pod_address_123";

    // Add a pod to the update list
    datastore.append_update_list(pod_address).unwrap();

    // Verify it was added
    let update_list = datastore.get_update_list().unwrap();
    assert!(update_list.pods.contains_key(pod_address));
    assert_eq!(update_list.pods[pod_address].len(), 0); // Should start with empty scratchpad list
}

#[test]
fn test_add_scratchpad_to_pod() {
    let (datastore, _temp_dir) = create_test_datastore();

    let pod_address = "test_pod_address_456";
    let scratchpad_address = "test_scratchpad_address_789";

    // Add a pod and then add a scratchpad to it
    datastore.append_update_list(pod_address).unwrap();
    datastore
        .add_scratchpad_to_pod(pod_address, scratchpad_address)
        .unwrap();

    // Verify the scratchpad was added
    let update_list = datastore.get_update_list().unwrap();
    assert!(update_list.pods.contains_key(pod_address));
    assert!(update_list.pods[pod_address].contains(&scratchpad_address.to_string()));
}

#[test]
fn test_append_pointer_removal() {
    let (datastore, _temp_dir) = create_test_datastore();

    let pointer_address = "test_pointer_address_abc";

    // Add a pointer for removal
    datastore
        .append_removal_list(pointer_address, "pointer")
        .unwrap();

    // Verify it was added to the removal list
    let update_list = datastore.get_update_list().unwrap();
    assert!(
        update_list
            .remove
            .pointers
            .contains(&pointer_address.to_string())
    );
}

#[test]
fn test_append_scratchpad_removal() {
    let (datastore, _temp_dir) = create_test_datastore();

    let scratchpad_address = "test_scratchpad_address_def";

    // Add a scratchpad for removal
    datastore
        .append_removal_list(scratchpad_address, "scratchpad")
        .unwrap();

    // Verify it was added to the removal list
    let update_list = datastore.get_update_list().unwrap();
    assert!(
        update_list
            .remove
            .scratchpads
            .contains(&scratchpad_address.to_string())
    );
}

#[test]
fn test_invalid_removal_type() {
    let (datastore, _temp_dir) = create_test_datastore();

    let address = "test_address_invalid";

    // Try to add with invalid type
    let result = datastore.append_removal_list(address, "invalid_type");
    assert!(result.is_err());
}

#[test]
fn test_duplicate_pod_entries() {
    let (datastore, _temp_dir) = create_test_datastore();

    let pod_address = "duplicate_pod_address";

    // Add the same pod multiple times
    datastore.append_update_list(pod_address).unwrap();
    datastore.append_update_list(pod_address).unwrap();
    datastore.append_update_list(pod_address).unwrap();

    // Should only appear once
    let update_list = datastore.get_update_list().unwrap();
    assert_eq!(update_list.pods.len(), 1);
    assert!(update_list.pods.contains_key(pod_address));
}

#[test]
fn test_duplicate_removal_entries() {
    let (datastore, _temp_dir) = create_test_datastore();

    let pointer_address = "duplicate_pointer_address";
    let scratchpad_address = "duplicate_scratchpad_address";

    // Add the same addresses for removal multiple times
    datastore
        .append_removal_list(pointer_address, "pointer")
        .unwrap();
    datastore
        .append_removal_list(pointer_address, "pointer")
        .unwrap();
    datastore
        .append_removal_list(scratchpad_address, "scratchpad")
        .unwrap();
    datastore
        .append_removal_list(scratchpad_address, "scratchpad")
        .unwrap();

    // Should only appear once each
    let update_list = datastore.get_update_list().unwrap();
    assert_eq!(update_list.remove.pointers.len(), 1);
    assert_eq!(update_list.remove.scratchpads.len(), 1);
    assert!(
        update_list
            .remove
            .pointers
            .contains(&pointer_address.to_string())
    );
    assert!(
        update_list
            .remove
            .scratchpads
            .contains(&scratchpad_address.to_string())
    );
}

#[test]
fn test_clear_update_list() {
    let (datastore, _temp_dir) = create_test_datastore();

    let pod_address = "test_pod_clear";
    let pointer_address = "test_pointer_clear";
    let scratchpad_address = "test_scratchpad_clear";

    // Add some entries
    datastore.append_update_list(pod_address).unwrap();
    datastore
        .append_removal_list(pointer_address, "pointer")
        .unwrap();
    datastore
        .append_removal_list(scratchpad_address, "scratchpad")
        .unwrap();

    // Verify they were added
    let update_list = datastore.get_update_list().unwrap();
    assert!(!update_list.pods.is_empty());
    assert!(!update_list.remove.pointers.is_empty());
    assert!(!update_list.remove.scratchpads.is_empty());

    // Clear the list
    datastore.clear_update_list().unwrap();

    // Verify it's empty
    let update_list = datastore.get_update_list().unwrap();
    assert!(update_list.pods.is_empty());
    assert!(update_list.remove.pointers.is_empty());
    assert!(update_list.remove.scratchpads.is_empty());
}

#[test]
fn test_json_file_format() {
    let (datastore, _temp_dir) = create_test_datastore();

    let pod_address = "test_pod_json";
    let scratchpad_address = "test_scratchpad_json";
    let pointer_address = "test_pointer_json";
    let removal_scratchpad = "test_removal_scratchpad";

    // Add various entries
    datastore.append_update_list(pod_address).unwrap();
    datastore
        .add_scratchpad_to_pod(pod_address, scratchpad_address)
        .unwrap();
    datastore
        .append_removal_list(pointer_address, "pointer")
        .unwrap();
    datastore
        .append_removal_list(removal_scratchpad, "scratchpad")
        .unwrap();

    // Read the JSON file directly and verify its structure
    let update_list_path = datastore.get_update_list_path();
    assert!(update_list_path.exists());
    assert!(update_list_path.extension().unwrap() == "json");

    let content = fs::read_to_string(update_list_path).unwrap();
    let parsed: UpdateList = serde_json::from_str(&content).unwrap();

    // Verify the structure matches what we expect
    assert!(parsed.pods.contains_key(pod_address));
    assert!(parsed.pods[pod_address].contains(&scratchpad_address.to_string()));
    assert!(
        parsed
            .remove
            .pointers
            .contains(&pointer_address.to_string())
    );
    assert!(
        parsed
            .remove
            .scratchpads
            .contains(&removal_scratchpad.to_string())
    );
}

#[test]
fn test_migration_from_empty_file() {
    let (datastore, _temp_dir) = create_test_datastore();

    // Create an empty update list file (simulating old format or empty file)
    let update_list_path = datastore.get_update_list_path();
    fs::write(&update_list_path, "").unwrap();

    // Should be able to read it as empty JSON structure
    let update_list = datastore.get_update_list().unwrap();
    assert!(update_list.pods.is_empty());
    assert!(update_list.remove.pointers.is_empty());
    assert!(update_list.remove.scratchpads.is_empty());
}

#[test]
fn test_complex_update_list_scenario() {
    let (datastore, _temp_dir) = create_test_datastore();

    // Create a complex scenario with multiple pods and removals
    let pod1 = "pod_address_1";
    let pod2 = "pod_address_2";
    let scratchpad1 = "scratchpad_1";
    let scratchpad2 = "scratchpad_2";
    let scratchpad3 = "scratchpad_3";
    let remove_pointer = "remove_pointer_1";
    let remove_scratchpad = "remove_scratchpad_1";

    // Add pods and scratchpads
    datastore.append_update_list(pod1).unwrap();
    datastore.append_update_list(pod2).unwrap();
    datastore.add_scratchpad_to_pod(pod1, scratchpad1).unwrap();
    datastore.add_scratchpad_to_pod(pod1, scratchpad2).unwrap();
    datastore.add_scratchpad_to_pod(pod2, scratchpad3).unwrap();

    // Add removals
    datastore
        .append_removal_list(remove_pointer, "pointer")
        .unwrap();
    datastore
        .append_removal_list(remove_scratchpad, "scratchpad")
        .unwrap();

    // Verify the complete structure
    let update_list = datastore.get_update_list().unwrap();

    // Check pods
    assert_eq!(update_list.pods.len(), 2);
    assert_eq!(update_list.pods[pod1].len(), 2);
    assert_eq!(update_list.pods[pod2].len(), 1);
    assert!(update_list.pods[pod1].contains(&scratchpad1.to_string()));
    assert!(update_list.pods[pod1].contains(&scratchpad2.to_string()));
    assert!(update_list.pods[pod2].contains(&scratchpad3.to_string()));

    // Check removals
    assert_eq!(update_list.remove.pointers.len(), 1);
    assert_eq!(update_list.remove.scratchpads.len(), 1);
    assert!(
        update_list
            .remove
            .pointers
            .contains(&remove_pointer.to_string())
    );
    assert!(
        update_list
            .remove
            .scratchpads
            .contains(&remove_scratchpad.to_string())
    );
}

#[test]
fn test_cross_removal_pod_to_pointer_removal() {
    let (datastore, _temp_dir) = create_test_datastore();

    let pod_address = "test_cross_removal_pod";

    // First add pod to update list
    datastore.append_update_list(pod_address).unwrap();

    // Verify it's in the pods section
    let update_list = datastore.get_update_list().unwrap();
    assert!(update_list.pods.contains_key(pod_address));
    assert!(update_list.remove.pointers.is_empty());

    // Now add the same address to pointer removal list
    datastore
        .append_removal_list(pod_address, "pointer")
        .unwrap();

    // Verify it's moved from pods to pointer removal
    let update_list = datastore.get_update_list().unwrap();
    assert!(!update_list.pods.contains_key(pod_address));
    assert!(
        update_list
            .remove
            .pointers
            .contains(&pod_address.to_string())
    );
}

#[test]
fn test_cross_removal_pointer_removal_to_pod() {
    let (datastore, _temp_dir) = create_test_datastore();

    let pod_address = "test_cross_removal_pointer";

    // First add pointer to removal list
    datastore
        .append_removal_list(pod_address, "pointer")
        .unwrap();

    // Verify it's in the pointer removal section
    let update_list = datastore.get_update_list().unwrap();
    assert!(
        update_list
            .remove
            .pointers
            .contains(&pod_address.to_string())
    );
    assert!(!update_list.pods.contains_key(pod_address));

    // Now add the same address to pods update list
    datastore.append_update_list(pod_address).unwrap();

    // Verify it's moved from pointer removal to pods
    let update_list = datastore.get_update_list().unwrap();
    assert!(update_list.pods.contains_key(pod_address));
    assert!(
        !update_list
            .remove
            .pointers
            .contains(&pod_address.to_string())
    );
}

#[test]
fn test_cross_removal_scratchpad_to_removal() {
    let (datastore, _temp_dir) = create_test_datastore();

    let pod_address = "test_cross_removal_scratchpad_pod";
    let scratchpad_address = "test_cross_removal_scratchpad";

    // First add pod and scratchpad to update list
    datastore.append_update_list(pod_address).unwrap();
    datastore
        .add_scratchpad_to_pod(pod_address, scratchpad_address)
        .unwrap();

    // Verify scratchpad is in the pod's scratchpad list
    let update_list = datastore.get_update_list().unwrap();
    assert!(update_list.pods[pod_address].contains(&scratchpad_address.to_string()));
    assert!(update_list.remove.scratchpads.is_empty());

    // Now add the scratchpad to removal list
    datastore
        .append_removal_list(scratchpad_address, "scratchpad")
        .unwrap();

    // Verify it's moved from pod's scratchpad list to scratchpad removal
    let update_list = datastore.get_update_list().unwrap();
    assert!(!update_list.pods[pod_address].contains(&scratchpad_address.to_string()));
    assert!(
        update_list
            .remove
            .scratchpads
            .contains(&scratchpad_address.to_string())
    );
}

#[test]
fn test_cross_removal_scratchpad_removal_to_pod() {
    let (datastore, _temp_dir) = create_test_datastore();

    let pod_address = "test_cross_removal_scratchpad_pod2";
    let scratchpad_address = "test_cross_removal_scratchpad2";

    // First add scratchpad to removal list
    datastore
        .append_removal_list(scratchpad_address, "scratchpad")
        .unwrap();

    // Verify it's in the scratchpad removal section
    let update_list = datastore.get_update_list().unwrap();
    assert!(
        update_list
            .remove
            .scratchpads
            .contains(&scratchpad_address.to_string())
    );

    // Now add the scratchpad to a pod's update list
    datastore
        .add_scratchpad_to_pod(pod_address, scratchpad_address)
        .unwrap();

    // Verify it's moved from scratchpad removal to pod's scratchpad list
    let update_list = datastore.get_update_list().unwrap();
    assert!(update_list.pods.contains_key(pod_address));
    assert!(update_list.pods[pod_address].contains(&scratchpad_address.to_string()));
    assert!(
        !update_list
            .remove
            .scratchpads
            .contains(&scratchpad_address.to_string())
    );
}

#[test]
fn test_update_list_comparison_with_free_addresses() {
    let (datastore, _temp_dir) = create_test_datastore();

    // Create some test addresses
    let pod_address1 = "pod_address_1";
    let pod_address2 = "pod_address_2";
    let scratchpad_address1 = "scratchpad_address_1";
    let scratchpad_address2 = "scratchpad_address_2";
    let scratchpad_address3 = "scratchpad_address_3";

    // Add pods and scratchpads to update list
    datastore.append_update_list(pod_address1).unwrap();
    datastore
        .add_scratchpad_to_pod(pod_address1, scratchpad_address1)
        .unwrap();
    datastore
        .add_scratchpad_to_pod(pod_address1, scratchpad_address2)
        .unwrap();

    datastore.append_update_list(pod_address2).unwrap();
    datastore
        .add_scratchpad_to_pod(pod_address2, scratchpad_address3)
        .unwrap();

    // Verify the update list structure
    let update_list = datastore.get_update_list().unwrap();
    assert_eq!(update_list.pods.len(), 2);
    assert!(update_list.pods.contains_key(pod_address1));
    assert!(update_list.pods.contains_key(pod_address2));
    assert_eq!(update_list.pods[pod_address1].len(), 2);
    assert_eq!(update_list.pods[pod_address2].len(), 1);

    // Verify specific scratchpad associations
    assert!(update_list.pods[pod_address1].contains(&scratchpad_address1.to_string()));
    assert!(update_list.pods[pod_address1].contains(&scratchpad_address2.to_string()));
    assert!(update_list.pods[pod_address2].contains(&scratchpad_address3.to_string()));
}
