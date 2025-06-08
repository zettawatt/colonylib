mod common;
use common::create_test_components;

#[test]
fn test_get_pods_at_depth() {
    let (_data_store, _key_store, mut graph, _temp_dir) = create_test_components();

    // Create a mock PodManager-like structure for testing
    let pod1 = "pod1_address";
    let pod2 = "pod2_address";
    let pod3 = "pod3_address";

    // Set up depths in the graph
    graph.update_pod_depth(pod1, 0).unwrap();
    graph.update_pod_depth(pod2, 1).unwrap();
    graph.update_pod_depth(pod3, 0).unwrap();

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
    let trig_data = format!(r#"
        @prefix ant: <ant://> .
            <ant://referenced_pod1> <ant://colonylib/vocabulary/0.1/predicate#addr_type> <ant://colonylib/vocabulary/0.1/object#pod_ref> .
            <ant://referenced_pod2> <ant://colonylib/vocabulary/0.1/predicate#addr_type> <ant://colonylib/vocabulary/0.1/object#pod_ref> .
            <ant://subject3> <ant://colonylib/vocabulary/0.1/predicate#name> "Test Name" .
    "#);

    // Load the test data
    graph.load_pod_into_graph(pod_address, &trig_data).unwrap();
    // graph.load_trig_data(&trig_data).unwrap();

    // Get references
    let references = graph.get_pod_references(pod_address).unwrap();

    // Should find the referenced pods
    assert!(references.contains(&"referenced_pod1".to_string()));
    assert!(references.contains(&"referenced_pod2".to_string()));

    // Should not contain vocabulary URIs
    assert!(!references.iter().any(|r| r.contains("/vocabulary/")));
}

#[test]
fn test_depth_update_logic() {
    let (_data_store, _key_store, mut graph, _temp_dir) = create_test_components();

    let pod_address = "test_depth_pod";

    // Initially no depth set (should return u64::MAX)
    let initial_depth = graph.get_pod_depth(pod_address).unwrap();
    assert_eq!(initial_depth, u64::MAX);

    // Set initial depth to 5
    graph.update_pod_depth(pod_address, 5).unwrap();
    assert_eq!(graph.get_pod_depth(pod_address).unwrap(), 5);

    // Try to set depth to 3 (should work since 3 < 5)
    graph.update_pod_depth(pod_address, 3).unwrap();
    assert_eq!(graph.get_pod_depth(pod_address).unwrap(), 3);

    // Try to set depth to 7 (should not change since 7 > 3)
    graph.update_pod_depth(pod_address, 7).unwrap();
    assert_eq!(graph.get_pod_depth(pod_address).unwrap(), 3);

    // Try to set depth to 1 (should work since 1 < 3)
    graph.update_pod_depth(pod_address, 1).unwrap();
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
    data_store.create_scratchpad_file(scratchpad_address).unwrap();

    // Set up pointer to point to scratchpad
    data_store.update_pointer_target(pod_address, scratchpad_address).unwrap();
    data_store.update_scratchpad_data(scratchpad_address, test_data).unwrap();

    // Verify the setup
    assert!(data_store.address_is_pointer(pod_address).unwrap());
    assert!(data_store.address_is_scratchpad(scratchpad_address).unwrap());

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

    let content = std::fs::read_to_string(update_list_path).unwrap();
    for addr in &addresses {
        assert!(content.contains(addr));
    }

    // Test duplicate prevention
    data_store.append_update_list("addr1").unwrap();
    let content = std::fs::read_to_string(data_store.get_update_list_path()).unwrap();
    let count = content.lines().filter(|line| *line == "addr1").count();
    assert_eq!(count, 1);
}

#[test]
fn test_graph_pod_entry_creation() {
    let (_data_store, _key_store, mut graph, _temp_dir) = create_test_components();

    let pod_address = "test_pod_entry";
    let scratchpad_address = "test_scratchpad_entry";

    // Create pod entry
    let trig_data = graph.add_pod_entry("test pod", pod_address, scratchpad_address).unwrap();

    // Verify the TriG data contains expected elements
    assert!(!trig_data.is_empty());
    // The function creates data about the scratchpad, not the pod address directly
    assert!(trig_data.contains(&format!("ant://{}", scratchpad_address)));
    // Check for the actual predicate URIs
    assert!(trig_data.contains("colonylib/vocabulary"));
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

    // Initially, no pointers or scratchpads should exist
    assert!(key_store.get_pointers().is_empty());
    assert!(key_store.get_scratchpads().is_empty());

    // Add keys and test that they exist
    key_store.add_pointer_key().unwrap();
    key_store.add_scratchpad_key().unwrap();

    assert!(!key_store.get_pointers().is_empty());
    assert!(!key_store.get_scratchpads().is_empty());
}

#[test]
fn test_error_handling() {
    let (data_store, _key_store, graph, _temp_dir) = create_test_components();

    let non_existent_address = "non_existent_address";

    // Test DataStore error handling
    assert!(data_store.get_pointer_target(non_existent_address).is_err());
    assert!(data_store.get_scratchpad_data(non_existent_address).is_err());

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
    let pod_iri = format!("ant://{}", pod_address);

    // Add searchable content
    graph.put_quad(
        "ant://file1",
        "ant://colonylib/vocabulary/0.1/predicate#name",
        "Important Document",
        Some(&pod_iri)
    ).unwrap();

    graph.put_quad(
        "ant://file1",
        "ant://colonylib/vocabulary/0.1/predicate#description",
        "This document contains important information",
        Some(&pod_iri)
    ).unwrap();

    graph.put_quad(
        "ant://file2",
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
        "http://schema.org/MediaObject",
        Some(&pod_iri)
    ).unwrap();

    // Test search directly on graph (without network calls)
    let search_results = graph.search_content("important", Some(10)).unwrap();
    let parsed_results: serde_json::Value = serde_json::from_str(&search_results).unwrap();

    // Verify results structure
    assert!(parsed_results.get("results").is_some());
    let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
    assert!(bindings.len() > 0);

    // Test type search
    let type_results = graph.search_by_type("http://schema.org/MediaObject", Some(10)).unwrap();
    let parsed_type_results: serde_json::Value = serde_json::from_str(&type_results).unwrap();
    let type_bindings = parsed_type_results["results"]["bindings"].as_array().unwrap();
    assert_eq!(type_bindings.len(), 1);

    // Test predicate search
    let predicate_results = graph.search_by_predicate(
        "ant://colonylib/vocabulary/0.1/predicate#name",
        Some(10)
    ).unwrap();
    let parsed_predicate_results: serde_json::Value = serde_json::from_str(&predicate_results).unwrap();
    let predicate_bindings = parsed_predicate_results["results"]["bindings"].as_array().unwrap();
    assert_eq!(predicate_bindings.len(), 1);
}

#[test]
fn test_structured_search_queries() {
    let (_data_store, _key_store, graph, _temp_dir) = create_test_components();

    // Add test data
    let pod_address = "test_structured_search";
    let pod_iri = format!("ant://{}", pod_address);

    graph.put_quad(
        "ant://media1",
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
        "http://schema.org/MediaObject",
        Some(&pod_iri)
    ).unwrap();

    graph.put_quad(
        "ant://media1",
        "ant://colonylib/vocabulary/0.1/predicate#name",
        "Test Video",
        Some(&pod_iri)
    ).unwrap();

    // Test text search directly on graph
    let text_results = graph.search_content("video", Some(10)).unwrap();
    let parsed_text_results: serde_json::Value = serde_json::from_str(&text_results).unwrap();
    let text_bindings = parsed_text_results["results"]["bindings"].as_array().unwrap();
    assert!(text_bindings.len() > 0);

    // Test type search directly on graph
    let type_results = graph.search_by_type("http://schema.org/MediaObject", Some(10)).unwrap();
    let parsed_type_results: serde_json::Value = serde_json::from_str(&type_results).unwrap();
    let type_bindings = parsed_type_results["results"]["bindings"].as_array().unwrap();
    assert_eq!(type_bindings.len(), 1);

    // Test predicate search directly on graph
    let predicate_results = graph.search_by_predicate(
        "ant://colonylib/vocabulary/0.1/predicate#name",
        Some(10)
    ).unwrap();
    let parsed_predicate_results: serde_json::Value = serde_json::from_str(&predicate_results).unwrap();
    let predicate_bindings = parsed_predicate_results["results"]["bindings"].as_array().unwrap();
    assert_eq!(predicate_bindings.len(), 1);

    // Test advanced search directly on graph
    let advanced_criteria = serde_json::json!({
        "text": "test",
        "type": "http://schema.org/MediaObject",
        "limit": 10
    });
    let advanced_results = graph.advanced_search(&advanced_criteria).unwrap();
    let parsed_advanced_results: serde_json::Value = serde_json::from_str(&advanced_results).unwrap();
    let advanced_bindings = parsed_advanced_results["results"]["bindings"].as_array().unwrap();
    assert!(advanced_bindings.len() > 0);
}

#[test]
fn test_search_error_handling() {
    let (_data_store, _key_store, graph, _temp_dir) = create_test_components();

    // Test search with empty text (should return no results)
    let empty_results = graph.search_content("", Some(10)).unwrap();
    let parsed_empty_results: serde_json::Value = serde_json::from_str(&empty_results).unwrap();
    let empty_bindings = parsed_empty_results["results"]["bindings"].as_array().unwrap();
    assert_eq!(empty_bindings.len(), 0);

    // Test search with non-existent text
    let no_results = graph.search_content("nonexistent_text_12345", Some(10)).unwrap();
    let parsed_no_results: serde_json::Value = serde_json::from_str(&no_results).unwrap();
    let no_bindings = parsed_no_results["results"]["bindings"].as_array().unwrap();
    assert_eq!(no_bindings.len(), 0);

    // Test search by non-existent type
    let no_type_results = graph.search_by_type("http://example.com/NonExistentType", Some(10)).unwrap();
    let parsed_no_type_results: serde_json::Value = serde_json::from_str(&no_type_results).unwrap();
    let no_type_bindings = parsed_no_type_results["results"]["bindings"].as_array().unwrap();
    assert_eq!(no_type_bindings.len(), 0);

    // Test search by non-existent predicate
    let no_pred_results = graph.search_by_predicate("http://example.com/nonexistent", Some(10)).unwrap();
    let parsed_no_pred_results: serde_json::Value = serde_json::from_str(&no_pred_results).unwrap();
    let no_pred_bindings = parsed_no_pred_results["results"]["bindings"].as_array().unwrap();
    assert_eq!(no_pred_bindings.len(), 0);

    // Test advanced search with empty criteria
    let empty_criteria = serde_json::json!({});
    let empty_advanced_results = graph.advanced_search(&empty_criteria).unwrap();
    let parsed_empty_advanced: serde_json::Value = serde_json::from_str(&empty_advanced_results).unwrap();
    // Should return all triples (if any exist) since no filters are applied
    assert!(parsed_empty_advanced.get("results").is_some());
}

// NOTE: this test can only be run if there is a local testnet running, so ignoring by default
#[ignore]
#[test]
fn test_data_splitting_helper_functions() {
    use colonylib::PodManager;
    use autonomi::{Client, Wallet};

    let (mut data_store, mut key_store, mut graph, _temp_dir) = create_test_components();

    // Create a mock PodManager for testing helper functions
    // We'll use a dummy client and wallet since we're only testing the helper functions
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (client, wallet) = rt.block_on(async {
        let client = Client::init_local().await.expect("Failed to create test client");
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
            assert!(!found_pod_ref && !found_other, "pod_index lines should come first");
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
    let test_data_for_chunking = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8\nLine 9\nLine 10\n".repeat(10);

    let chunks = pod_manager.split_data_into_chunks(&test_data_for_chunking, chunk_size);

    // Verify that chunks were created
    assert!(chunks.len() > 1, "Should have created multiple chunks");

    // Verify that no chunk exceeds the size limit
    for (i, chunk) in chunks.iter().enumerate() {
        assert!(chunk.len() <= chunk_size,
            "Chunk {} exceeds size limit: {} bytes", i, chunk.len());
    }

    // Verify that all data is preserved when chunks are concatenated
    let reconstructed = chunks.join("");
    assert_eq!(reconstructed.trim(), test_data_for_chunking.trim(),
        "Reconstructed data should match original");

    // Test with very large single line
    let large_line = "A".repeat(200); // Larger than chunk size
    let chunks_large = pod_manager.split_data_into_chunks(&large_line, chunk_size);

    // Should split the large line into multiple chunks
    assert!(chunks_large.len() > 1, "Should split large line into multiple chunks");

    // Verify total size is preserved (accounting for the newline that gets added)
    let total_size: usize = chunks_large.iter().map(|c| c.len()).sum();
    let expected_size = large_line.len() + 1; // +1 for the newline that gets added
    assert_eq!(total_size, expected_size, "Total size should be preserved (with newline)");

    println!("Data splitting helper functions test completed successfully!");
    println!("Created {} chunks from test data", chunks.len());
    println!("Created {} chunks from large line", chunks_large.len());
}
