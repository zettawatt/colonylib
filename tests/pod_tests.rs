mod common;
use colonylib::DataStore;
use common::create_test_components;

#[test]
fn test_get_pods_at_depth() {
    let (_data_store, _key_store, mut graph, _temp_dir) = create_test_components();

    // Create a mock PodManager-like structure for testing
    let pod1 = "pod1_address";
    let pod2 = "pod2_address";
    let pod3 = "pod3_address";

    // Set up depths in the graph
    graph.force_set_pod_depth(pod1, "config1", 0).unwrap();
    graph.force_set_pod_depth(pod2, "config2", 1).unwrap();
    graph.force_set_pod_depth(pod3, "config3", 0).unwrap();

    // Test getting pods at different depths
    let pods_at_depth_0 = graph.get_pods_at_depth(0).unwrap();
    assert_eq!(pods_at_depth_0.len(), 2);
    assert!(pods_at_depth_0.contains(&pod1.to_string()));
    assert!(pods_at_depth_0.contains(&pod3.to_string()));

    let pods_at_depth_1 = graph.get_pods_at_depth(1).unwrap();
    assert_eq!(pods_at_depth_1.len(), 1);
    assert!(pods_at_depth_1.contains(&pod2.to_string()));
}

#[test]
fn test_pod_reference_extraction() {
    let (_data_store, _key_store, mut graph, _temp_dir) = create_test_components();

    let pod_address = "test_pod";

    // Create test TriG data with references
    let trig_data = r#"
        @prefix ant: <ant://> .
            <ant://referenced_pod1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <ant://colonylib/v1/ref> .
            <ant://referenced_pod2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <ant://colonylib/v1/ref> .
            <ant://subject3> <http://schema.org/name> "Test Name" .
    "#.to_string();

    // Load the test data
    graph.load_pod_into_graph(pod_address, &trig_data).unwrap();
    // graph.load_trig_data(&trig_data).unwrap();

    // Get references
    let references = graph.get_pod_references(pod_address).unwrap();

    // Should find the referenced pods
    assert!(references.contains(&"referenced_pod1".to_string()));
    assert!(references.contains(&"referenced_pod2".to_string()));

    // Should not contain vocabulary URIs
    assert!(!references.iter().any(|r| r.contains("/v1/")));
}

#[test]
fn test_depth_update_logic() {
    let (_data_store, _key_store, mut graph, _temp_dir) = create_test_components();

    let pod_address = "test_depth_pod";

    // Initially no depth set (should return u64::MAX)
    let initial_depth = graph.get_pod_depth(pod_address).unwrap();
    assert_eq!(initial_depth, u64::MAX);

    // Set initial depth to 5
    graph
        .update_pod_depth(pod_address, "test_config", 5)
        .unwrap();
    assert_eq!(graph.get_pod_depth(pod_address).unwrap(), 5);

    // Try to set depth to 3 (should work since 3 < 5)
    graph
        .update_pod_depth(pod_address, "test_config", 3)
        .unwrap();
    assert_eq!(graph.get_pod_depth(pod_address).unwrap(), 3);

    // Try to set depth to 7 (should not change since 7 > 3)
    graph
        .update_pod_depth(pod_address, "test_config", 7)
        .unwrap();
    assert_eq!(graph.get_pod_depth(pod_address).unwrap(), 3);

    // Try to set depth to 1 (should work since 1 < 3)
    graph
        .update_pod_depth(pod_address, "test_config", 1)
        .unwrap();
    assert_eq!(graph.get_pod_depth(pod_address).unwrap(), 1);
}

#[test]
fn test_data_store_integration() {
    let (data_store, _key_store, _graph, _temp_dir) = create_test_components();

    let pod_address = "integration_test_pod";
    let scratchpad_address = "integration_test_scratchpad";
    let test_data = "test pod data content";

    // Create pointer and scratchpad files
    data_store.create_pointer_file(pod_address).unwrap();
    data_store
        .create_scratchpad_file(scratchpad_address)
        .unwrap();

    // Set up pointer to point to scratchpad
    data_store
        .update_pointer_target(pod_address, scratchpad_address)
        .unwrap();
    data_store
        .update_scratchpad_data(scratchpad_address, test_data)
        .unwrap();

    // Verify the setup
    assert!(data_store.address_is_pointer(pod_address).unwrap());
    assert!(
        data_store
            .address_is_scratchpad(scratchpad_address)
            .unwrap()
    );

    let target = data_store.get_pointer_target(pod_address).unwrap();
    assert_eq!(target, scratchpad_address);

    let retrieved_data = data_store.get_scratchpad_data(scratchpad_address).unwrap();
    assert_eq!(retrieved_data, test_data);
}

#[test]
fn test_update_list_functionality() {
    let (data_store, _key_store, _graph, _temp_dir) = create_test_components();

    let addresses = vec!["addr1", "addr2", "addr3"];

    // Add addresses to update list
    for addr in &addresses {
        data_store.append_update_list(addr).unwrap();
    }

    // Verify update list file exists and contains all addresses
    let update_list_path = data_store.get_update_list_path();
    assert!(update_list_path.exists());

    // Get the update list using the proper API
    let update_list = data_store.get_update_list().unwrap();
    for addr in &addresses {
        assert!(update_list.pods.contains_key(*addr));
    }

    // Test duplicate prevention
    data_store.append_update_list("addr1").unwrap();
    let update_list = data_store.get_update_list().unwrap();
    assert_eq!(update_list.pods.len(), 3); // Should still be 3, not 4
    assert!(update_list.pods.contains_key("addr1"));
}

#[test]
fn test_graph_pod_entry_creation() {
    let (_data_store, _key_store, mut graph, _temp_dir) = create_test_components();

    let pod_address = "test_pod_entry";
    let scratchpad_address = "test_scratchpad_entry";

    // Create pod entry
    let (trig_data, config_data) = graph
        .add_pod_entry(
            "test pod",
            pod_address,
            scratchpad_address,
            "test_config",
            "test_config_scratchpad",
            0,
        )
        .unwrap();

    // Verify the TriG data contains expected elements
    assert!(!trig_data.is_empty());
    assert!(!config_data.is_empty());

    // Convert Vec<u8> to String for checking contents
    let trig_string = String::from_utf8(trig_data).unwrap();
    let config_string = String::from_utf8(config_data).unwrap();

    // The function creates data about the scratchpad, not the pod address directly
    assert!(trig_string.contains(&format!("ant://{scratchpad_address}")));
    // Check for the actual predicate URIs
    assert!(trig_string.contains("colonylib/v1") || config_string.contains("colonylib/v1"));
    // Note: depth is stored in the default graph, not in the pod's named graph
    // so it won't appear in the TriG output for the specific pod graph

    // Verify that the depth was actually set by querying it directly
    let depth = graph.get_pod_depth(pod_address).unwrap();
    assert_eq!(depth, 0); // Initial depth should be 0
}

#[test]
fn test_keystore_integration() {
    let (_data_store, mut key_store, _graph, _temp_dir) = create_test_components();

    // Test that keystore was created with the test mnemonic
    let expected_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    assert_eq!(key_store.get_seed_phrase(), expected_mnemonic);

    // Initially, one configuration pointer and one configuration scratchpad should exist
    assert_eq!(key_store.get_pointers().len(), 1);
    assert_eq!(key_store.get_scratchpads().len(), 1);

    // Add additional keys and test that they exist
    key_store.add_pointer_key().unwrap();
    key_store.add_scratchpad_key().unwrap();

    assert_eq!(key_store.get_pointers().len(), 2);
    assert_eq!(key_store.get_scratchpads().len(), 2);
}

#[test]
fn test_error_handling() {
    let (data_store, _key_store, graph, _temp_dir) = create_test_components();

    let non_existent_address = "non_existent_address";

    // Test DataStore error handling
    assert!(data_store.get_pointer_target(non_existent_address).is_err());
    assert!(
        data_store
            .get_scratchpad_data(non_existent_address)
            .is_err()
    );

    // Test Graph operations with non-existent data
    let depth = graph.get_pod_depth(non_existent_address).unwrap();
    assert_eq!(depth, u64::MAX); // Should return MAX for non-existent pods

    let references = graph.get_pod_references(non_existent_address).unwrap();
    assert!(references.is_empty()); // Should return empty vec for non-existent pods
}

#[test]
fn test_search_functionality() {
    let (_data_store, _key_store, graph, _temp_dir) = create_test_components();

    // Add test data to the graph
    let pod_address = "test_search_pod";
    let pod_iri = "ant://".to_string() + pod_address;

    // Add searchable content
    graph
        .put_quad(
            "ant://file1",
            "ant://colonylib/vocabulary/0.1/predicate#name",
            "Important Document",
            Some(&pod_iri),
        )
        .unwrap();

    graph
        .put_quad(
            "ant://file1",
            "ant://colonylib/vocabulary/0.1/predicate#description",
            "This document contains important information",
            Some(&pod_iri),
        )
        .unwrap();

    graph
        .put_quad(
            "ant://file2",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            "http://schema.org/MediaObject",
            Some(&pod_iri),
        )
        .unwrap();

    // Test search directly on graph (without network calls)
    let search_results = graph.search_content("important", Some(10)).unwrap();
    let parsed_results: serde_json::Value = serde_json::from_str(&search_results).unwrap();

    // Verify results structure
    assert!(parsed_results.get("results").is_some());
    let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
    assert!(!bindings.is_empty());

    // Test type search
    let type_results = graph
        .search_by_type("http://schema.org/MediaObject", Some(10))
        .unwrap();
    let parsed_type_results: serde_json::Value = serde_json::from_str(&type_results).unwrap();
    let type_bindings = parsed_type_results["results"]["bindings"]
        .as_array()
        .unwrap();
    assert_eq!(type_bindings.len(), 1);

    // Test predicate search
    let predicate_results = graph
        .search_by_predicate("ant://colonylib/vocabulary/0.1/predicate#name", Some(10))
        .unwrap();
    let parsed_predicate_results: serde_json::Value =
        serde_json::from_str(&predicate_results).unwrap();
    let predicate_bindings = parsed_predicate_results["results"]["bindings"]
        .as_array()
        .unwrap();
    assert_eq!(predicate_bindings.len(), 1);
}

#[test]
fn test_structured_search_queries() {
    let (_data_store, _key_store, graph, _temp_dir) = create_test_components();

    // Add test data
    let pod_address = "test_structured_search";
    let pod_iri = "ant://".to_string() + pod_address;

    graph
        .put_quad(
            "ant://media1",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            "http://schema.org/MediaObject",
            Some(&pod_iri),
        )
        .unwrap();

    graph
        .put_quad(
            "ant://media1",
            "ant://colonylib/vocabulary/0.1/predicate#name",
            "Test Video",
            Some(&pod_iri),
        )
        .unwrap();

    // Test text search directly on graph
    let text_results = graph.search_content("video", Some(10)).unwrap();
    let parsed_text_results: serde_json::Value = serde_json::from_str(&text_results).unwrap();
    let text_bindings = parsed_text_results["results"]["bindings"]
        .as_array()
        .unwrap();
    assert!(!text_bindings.is_empty());

    // Test type search directly on graph
    let type_results = graph
        .search_by_type("http://schema.org/MediaObject", Some(10))
        .unwrap();
    let parsed_type_results: serde_json::Value = serde_json::from_str(&type_results).unwrap();
    let type_bindings = parsed_type_results["results"]["bindings"]
        .as_array()
        .unwrap();
    assert_eq!(type_bindings.len(), 1);

    // Test predicate search directly on graph
    let predicate_results = graph
        .search_by_predicate("ant://colonylib/vocabulary/0.1/predicate#name", Some(10))
        .unwrap();
    let parsed_predicate_results: serde_json::Value =
        serde_json::from_str(&predicate_results).unwrap();
    let predicate_bindings = parsed_predicate_results["results"]["bindings"]
        .as_array()
        .unwrap();
    assert_eq!(predicate_bindings.len(), 1);

    // Test advanced search directly on graph

    let query = r#"
        SELECT DISTINCT ?subject ?predicate ?object ?graph WHERE {{
            GRAPH ?graph {{
                ?subject ?predicate ?object .
                FILTER(isLiteral(?object) && CONTAINS(LCASE(STR(?object)), LCASE("test")))
                ?subject <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/MediaObject> .
            }}
        }}
        ORDER BY ?graph ?subject
        LIMIT 10
    "#;
    let advanced_results = graph.advanced_search(query).unwrap();
    let parsed_advanced_results: serde_json::Value =
        serde_json::from_str(&advanced_results).unwrap();
    let advanced_bindings = parsed_advanced_results["results"]["bindings"]
        .as_array()
        .unwrap();
    assert!(!advanced_bindings.is_empty());
}

#[test]
fn test_search_error_handling() {
    let (_data_store, _key_store, graph, _temp_dir) = create_test_components();

    // Test search with empty text (should return empty array)
    let empty_results = graph.search_content("", Some(10)).unwrap();
    assert_eq!(
        empty_results, "[]",
        "Empty search should return empty array"
    );

    // Test search with non-existent text
    let no_results = graph
        .search_content("nonexistent_text_12345", Some(10))
        .unwrap();
    let parsed_no_results: serde_json::Value = serde_json::from_str(&no_results).unwrap();
    let no_bindings = parsed_no_results["results"]["bindings"].as_array().unwrap();
    assert_eq!(no_bindings.len(), 0);

    // Test search by non-existent type
    let no_type_results = graph
        .search_by_type("http://example.com/NonExistentType", Some(10))
        .unwrap();
    let parsed_no_type_results: serde_json::Value = serde_json::from_str(&no_type_results).unwrap();
    let no_type_bindings = parsed_no_type_results["results"]["bindings"]
        .as_array()
        .unwrap();
    assert_eq!(no_type_bindings.len(), 0);

    // Test search by non-existent predicate
    let no_pred_results = graph
        .search_by_predicate("http://example.com/nonexistent", Some(10))
        .unwrap();
    let parsed_no_pred_results: serde_json::Value = serde_json::from_str(&no_pred_results).unwrap();
    let no_pred_bindings = parsed_no_pred_results["results"]["bindings"]
        .as_array()
        .unwrap();
    assert_eq!(no_pred_bindings.len(), 0);

    // Test advanced search with empty criteria
    let empty_criteria = "";
    let empty_advanced_results = graph.advanced_search(empty_criteria).unwrap();
    let parsed_empty_advanced: serde_json::Value =
        serde_json::from_str(&empty_advanced_results).unwrap();
    // Should return all triples (if any exist) since no filters are applied
    assert!(parsed_empty_advanced.get("results").is_some());
}

// NOTE: this test can only be run if there is a local testnet running, so ignoring by default
#[ignore]
#[test]
fn test_data_splitting_helper_functions() {
    use autonomi::{Client, Wallet};
    use colonylib::PodManager;

    let (mut data_store, mut key_store, mut graph, _temp_dir) = create_test_components();

    // Create a mock PodManager for testing helper functions
    // We'll use a dummy client and wallet since we're only testing the helper functions
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (client, wallet) = rt.block_on(async {
        let client = Client::init_local()
            .await
            .expect("Failed to create test client");
        let evm_network = client.evm_network().clone();
        let private_key = "0x1234567890123456789012345678901234567890123456789012345678901234";
        let wallet = Wallet::new_from_private_key(evm_network, private_key)
            .expect("Failed to create test wallet");
        (client, wallet)
    });

    let pod_manager = rt.block_on(async {
        PodManager::new(client, &wallet, &mut data_store, &mut key_store, &mut graph)
            .await
            .expect("Failed to create PodManager")
    });

    // Test sort_graph_data function
    let test_data = r#"
<ant://subject1> <http://schema.org/name> "Test Name" .
<ant://scratchpad1> <ant://colonylib/vocabulary/0.1/predicate#pod_index> "0" .
<ant://subject2> <http://schema.org/description> "Test Description" .
<ant://pod_ref1> <ant://colonylib/vocabulary/0.1/object#pod_ref> "reference" .
<ant://subject3> <http://schema.org/type> "Dataset" .
<ant://scratchpad2> <ant://colonylib/vocabulary/0.1/predicate#pod_index> "1" .
"#;

    let sorted_data = pod_manager.sort_graph_data(test_data);
    let lines: Vec<&str> = sorted_data.lines().collect();

    // Verify that pod_index lines come first
    let mut found_pod_index = false;
    let mut found_pod_ref = false;
    let mut found_other = false;

    for line in &lines {
        if line.contains("pod_index") {
            assert!(
                !found_pod_ref && !found_other,
                "pod_index lines should come first"
            );
            found_pod_index = true;
        } else if line.contains("pod_ref") {
            assert!(!found_other, "pod_ref lines should come before other lines");
            found_pod_ref = true;
        } else if !line.trim().is_empty() {
            found_other = true;
        }
    }

    assert!(found_pod_index, "Should have found pod_index lines");

    // Test split_data_into_chunks function
    let chunk_size = 100; // Small chunk size for testing
    let test_data_for_chunking =
        "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8\nLine 9\nLine 10\n"
            .repeat(10);

    let chunks = pod_manager.split_data_into_chunks(&test_data_for_chunking, chunk_size);

    // Verify that chunks were created
    assert!(chunks.len() > 1, "Should have created multiple chunks");

    // Verify that no chunk exceeds the size limit
    for (i, chunk) in chunks.iter().enumerate() {
        assert!(
            chunk.len() <= chunk_size,
            "Chunk {} exceeds size limit: {} bytes",
            i,
            chunk.len()
        );
    }

    // Verify that each chunk starts with a timestamp comment
    for (i, chunk) in chunks.iter().enumerate() {
        assert!(
            chunk.starts_with('#'),
            "Chunk {} should start with timestamp comment",
            i
        );
        // Verify the timestamp comment format (should be <= 37 bytes: '#' + RFC3339 + '\n')
        let lines: Vec<&str> = chunk.lines().collect();
        if !lines.is_empty() {
            let timestamp_line = lines[0];
            assert!(
                timestamp_line.starts_with('#'),
                "First line should be timestamp comment"
            );
            assert!(
                timestamp_line.len() + 1 <= 37, // +1 for the newline
                "Timestamp comment should be at most 37 bytes including newline, got {}",
                timestamp_line.len() + 1
            );
        }
    }

    // Verify that original data is preserved when chunks are concatenated (after removing timestamp comments)
    let reconstructed_without_timestamps: String = chunks
        .iter()
        .map(|chunk| {
            // Remove the first line (timestamp comment) from each chunk
            let lines: Vec<&str> = chunk.lines().collect();
            if lines.len() > 1 {
                lines[1..].join("\n") + "\n"
            } else if lines.len() == 1 && !lines[0].starts_with('#') {
                // Handle case where chunk only contains data (no timestamp)
                chunk.clone()
            } else {
                // Chunk only contains timestamp comment
                String::new()
            }
        })
        .collect();

    assert_eq!(
        reconstructed_without_timestamps.trim(),
        test_data_for_chunking.trim(),
        "Reconstructed data (without timestamps) should match original"
    );

    // Test with very large single line
    let large_line = "A".repeat(200); // Larger than chunk size
    let chunks_large = pod_manager.split_data_into_chunks(&large_line, chunk_size);

    // Should split the large line into multiple chunks
    assert!(
        chunks_large.len() > 1,
        "Should split large line into multiple chunks"
    );

    // Verify each chunk has timestamp comment and doesn't exceed size limit
    for (i, chunk) in chunks_large.iter().enumerate() {
        assert!(
            chunk.len() <= chunk_size,
            "Chunk {} exceeds size limit: {} bytes",
            i,
            chunk.len()
        );
        assert!(
            chunk.starts_with('#'),
            "Large line chunk {} should start with timestamp comment",
            i
        );
    }

    println!("Data splitting helper functions test completed successfully!");
    println!("Created {} chunks from test data", chunks.len());
    println!("Created {} chunks from large line", chunks_large.len());
}

#[tokio::test]
async fn test_pod_manager_browse_search() {
    let (_data_store, _key_store, graph, _temp_dir) = create_test_components();

    // Create test pods at different depths
    let pod1_address = "browse_pod1";
    let pod2_address = "browse_pod2";
    let pod1_iri = format!("ant://{pod1_address}");
    let pod2_iri = format!("ant://{pod2_address}");

    // Add pod depth information
    graph
        .put_quad(
            &pod1_iri,
            "ant://colonylib/v1/depth",
            "0",
            Some("ant://config"),
        )
        .unwrap();
    graph
        .put_quad(
            &pod2_iri,
            "ant://colonylib/v1/depth",
            "1",
            Some("ant://config"),
        )
        .unwrap();

    // Add subjects with names
    graph
        .put_quad(
            "ant://browse_subject1",
            "http://schema.org/name",
            "Browse Subject 1",
            Some(&pod1_iri),
        )
        .unwrap();
    graph
        .put_quad(
            "ant://browse_subject1",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            "http://schema.org/MediaObject",
            Some(&pod1_iri),
        )
        .unwrap();
    graph
        .put_quad(
            "ant://browse_subject1",
            "http://schema.org/description",
            "First browse subject",
            Some(&pod1_iri),
        )
        .unwrap();

    graph
        .put_quad(
            "ant://browse_subject2",
            "http://schema.org/name",
            "Browse Subject 2",
            Some(&pod2_iri),
        )
        .unwrap();

    // Test browse search directly on graph
    let browse_results = graph.browse(Some(10)).unwrap();
    let parsed_results: serde_json::Value = serde_json::from_str(&browse_results).unwrap();

    // Verify results structure
    assert!(parsed_results.get("results").is_some());
    let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
    assert!(!bindings.is_empty(), "Browse should return results");

    // Verify that subjects are present
    let subject_names: Vec<String> = bindings
        .iter()
        .filter_map(|binding| {
            binding
                .get("name")
                .and_then(|name| name.get("value"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    assert!(
        subject_names.contains(&"Browse Subject 1".to_string()),
        "Should contain Browse Subject 1"
    );
    assert!(
        subject_names.contains(&"Browse Subject 2".to_string()),
        "Should contain Browse Subject 2"
    );

    println!("Browse search test completed successfully!");
    println!("Found {} subjects in browse results", bindings.len());
}

// Test active wallet functionality using KeyStore and DataStore directly
#[test]
fn test_keystore_datastore_active_wallet_integration() {
    let (data_store, mut key_store, _graph, _temp_dir) = create_test_components();

    let wallet_name = "test_wallet";
    let wallet_key = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";

    // Add a wallet key to the key store
    key_store.add_wallet_key(wallet_name, wallet_key).unwrap();

    // Set it as active using key store
    let (returned_name, returned_address) = key_store.set_active_wallet(wallet_name).unwrap();

    // Persist to data store
    data_store
        .set_active_wallet(&returned_name, &returned_address)
        .unwrap();

    // Verify we can retrieve it from data store
    let (get_name, get_address) = data_store.get_active_wallet().unwrap();

    assert_eq!(get_name, returned_name);
    assert_eq!(get_address, returned_address);
    assert_eq!(get_name, wallet_name);
    assert!(!get_address.is_empty());
    assert!(get_address.starts_with("0x"));
}

#[test]
fn test_keystore_active_wallet_nonexistent() {
    let (_data_store, mut key_store, _graph, _temp_dir) = create_test_components();

    // Try to set a wallet that doesn't exist
    let result = key_store.set_active_wallet("nonexistent_wallet");
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert!(error.to_string().contains("not found"));
}

#[test]
fn test_datastore_active_wallet_not_set() {
    let (data_store, _key_store, _graph, _temp_dir) = create_test_components();

    // Try to get active wallet when none is set
    let result = data_store.get_active_wallet();
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert!(error.to_string().contains("Active wallet file not found"));
}

#[test]
fn test_active_wallet_persistence_across_instances() {
    let (data_store, mut key_store, _graph, temp_dir) = create_test_components();

    let wallet_name = "persistent_wallet";
    let wallet_key = "0xfeedbeeffeedbeeffeedbeeffeedbeeffeedbeeffeedbeeffeedbeeffeedbeef";

    // Add and set active wallet
    key_store.add_wallet_key(wallet_name, wallet_key).unwrap();
    let (set_name, set_address) = key_store.set_active_wallet(wallet_name).unwrap();
    data_store
        .set_active_wallet(&set_name, &set_address)
        .unwrap();

    // Create a new DataStore instance with the same directory
    let data_dir = temp_dir.path().join("data");
    let pods_dir = temp_dir.path().join("pods");
    let downloads_dir = temp_dir.path().join("downloads");

    let new_data_store = DataStore::from_paths(data_dir, pods_dir, downloads_dir).unwrap();

    // The active wallet should persist
    let (get_name, get_address) = new_data_store.get_active_wallet().unwrap();

    assert_eq!(get_name, set_name);
    assert_eq!(get_address, set_address);
}
