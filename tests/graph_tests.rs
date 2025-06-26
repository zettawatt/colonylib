mod common;
use common::create_test_graph;

macro_rules! PREDICATE {
    ($e:expr) => {
        concat!("ant://colonylib/vocabulary/", "0.1/", "predicate#", $e)
    };
}

const HAS_POD_INDEX: &str = PREDICATE!("pod_index");

#[test]
fn test_graph_creation() {
    let (_graph, _temp_dir) = create_test_graph();
    // Graph should be created successfully
    assert!(true); // If we get here, graph creation worked
}

#[test]
fn test_add_pod_entry() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod_address = "1234567890abcdef";
    let scratchpad_address = "abcdef1234567890";
    let configuration_address = "config1234567890";
    let configuration_scratchpad_address = "config_scratchpad1234567890";
    let pod_name = "Test Pod";

    let result = graph.add_pod_entry(pod_name, pod_address, scratchpad_address, configuration_address, configuration_scratchpad_address, 0);
    assert!(result.is_ok(), "Failed to add pod entry: {:?}", result.err());

    let (trig_data, config_data) = result.unwrap();
    assert!(!trig_data.is_empty(), "TriG data should not be empty");
    assert!(!config_data.is_empty(), "Configuration data should not be empty");

    // Convert Vec<u8> to String for checking contents
    let trig_string = String::from_utf8(trig_data).unwrap();
    assert!(trig_string.contains(scratchpad_address), "TriG data should contain the scratchpad address");
}

#[test]
fn test_pod_depth_operations() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod_123";
    let configuration_address = "test_config_123";

    // Initially, pod should have no depth (returns u64::MAX)
    let initial_depth = graph.get_pod_depth(pod_address).unwrap();
    assert_eq!(initial_depth, u64::MAX);

    // Set depth to 0
    graph.update_pod_depth(pod_address, configuration_address, 0).unwrap();
    let depth = graph.get_pod_depth(pod_address).unwrap();
    assert_eq!(depth, 0);

    // Try to set depth to 2 (should NOT work since 2 > 0, depth should remain 0)
    graph.update_pod_depth(pod_address, configuration_address, 2).unwrap();
    let depth = graph.get_pod_depth(pod_address).unwrap();
    assert_eq!(depth, 0); // Should still be 0 since we only update to smaller depths

    // Try to set depth to 1 (should NOT work since 1 > 0, depth should remain 0)
    graph.update_pod_depth(pod_address, configuration_address, 1).unwrap();
    let depth = graph.get_pod_depth(pod_address).unwrap();
    assert_eq!(depth, 0); // Should still be 0

    // Now let's test with a higher initial depth
    // First set depth to 5
    graph.update_pod_depth(pod_address, configuration_address, 5).unwrap(); // This won't work since 5 > 0
    let depth = graph.get_pod_depth(pod_address).unwrap();
    assert_eq!(depth, 0); // Should still be 0

    // Let's start fresh with a new pod to test the depth logic properly
    let pod_address2 = "test_pod_456";
    let configuration_address2 = "test_config_456";

    // Set initial depth to 5 (this should work since no depth exists)
    graph.update_pod_depth(pod_address2, configuration_address2, 5).unwrap();
    let depth = graph.get_pod_depth(pod_address2).unwrap();
    assert_eq!(depth, 5);

    // Try to set depth to 3 (should work since 3 < 5)
    graph.update_pod_depth(pod_address2, configuration_address2, 3).unwrap();
    let depth = graph.get_pod_depth(pod_address2).unwrap();
    assert_eq!(depth, 3);

    // Try to set depth to 7 (should not change since 7 > 3)
    graph.update_pod_depth(pod_address2, configuration_address2, 7).unwrap();
    let depth = graph.get_pod_depth(pod_address2).unwrap();
    assert_eq!(depth, 3); // Should still be 3
}

#[test]
fn test_get_pods_at_depth() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod1 = "pod1_address";
    let pod2 = "pod2_address";
    let pod3 = "pod3_address";
    let config1 = "config1_address";
    let config2 = "config2_address";
    let config3 = "config3_address";

    // Set different depths
    graph.update_pod_depth(pod1, config1, 0).unwrap();
    graph.update_pod_depth(pod2, config2, 1).unwrap();
    graph.update_pod_depth(pod3, config3, 0).unwrap();

    // Get pods at depth 0
    let pods_at_depth_0 = graph.get_pods_at_depth(0).unwrap();
    assert_eq!(pods_at_depth_0.len(), 2);
    assert!(pods_at_depth_0.contains(&pod1.to_string()));
    assert!(pods_at_depth_0.contains(&pod3.to_string()));

    // Get pods at depth 1
    let pods_at_depth_1 = graph.get_pods_at_depth(1).unwrap();
    assert_eq!(pods_at_depth_1.len(), 1);
    assert!(pods_at_depth_1.contains(&pod2.to_string()));

    // Get pods at depth 2 (should be empty)
    let pods_at_depth_2 = graph.get_pods_at_depth(2).unwrap();
    assert_eq!(pods_at_depth_2.len(), 0);
}

#[test]
fn test_get_pod_references() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod";

    // Create a pod with some test data that includes references
    let trig_data = format!(r#"
        @prefix ant: <ant://> .
            <ant://referenced_pod1> <ant://colonylib/vocabulary/0.1/predicate#addr_type> <ant://colonylib/vocabulary/0.1/object#pod_ref> .
            <ant://referenced_pod2> <ant://colonylib/vocabulary/0.1/predicate#addr_type> <ant://colonylib/vocabulary/0.1/object#pod_ref> .
            <ant://{}> <ant://colonylib/vocabulary/0.1/predicate#name> "Some Name" .
    "#, pod_address);

    // Load the test data
    graph.load_pod_into_graph(pod_address, &trig_data).unwrap();

    // Get references
    let references = graph.get_pod_references(pod_address).unwrap();

    // Should find the referenced pods but not vocabulary URIs
    assert!(references.contains(&"referenced_pod1".to_string()));
    assert!(references.contains(&"referenced_pod2".to_string()));

    // Should not contain vocabulary URIs or the pod itself
    assert!(!references.iter().any(|r| r.contains("/vocabulary/")));
    assert!(!references.contains(&format!("ant://{}", pod_address)));
}

#[test]
fn test_put_quad() {
    let (graph, _temp_dir) = create_test_graph();

    let subject = "ant://test_subject";
    let predicate = "ant://colonylib/vocabulary/0.1/predicate#test";
    let object = "test_value";

    let result = graph.put_quad(subject, predicate, object, None);
    assert!(result.is_ok());

    // Test with named graph
    let graph_name = "ant://test_graph";
    let result = graph.put_quad(subject, predicate, object, Some(graph_name));
    assert!(result.is_ok());
}

#[test]
fn test_search_content() {
    let (graph, _temp_dir) = create_test_graph();

    // Add some test data
    let pod_address = "test_pod";
    let pod_iri = format!("ant://{}", pod_address);

    // Add test triples with searchable content
    graph.put_quad(
        "ant://subject1",
        "ant://colonylib/vocabulary/0.1/predicate#name",
        "Test Document",
        Some(&pod_iri)
    ).unwrap();

    graph.put_quad(
        "ant://subject2",
        "ant://colonylib/vocabulary/0.1/predicate#description",
        "This is a test description with searchable content",
        Some(&pod_iri)
    ).unwrap();

    graph.put_quad(
        "ant://subject3",
        "ant://colonylib/vocabulary/0.1/predicate#name",
        "Another Document",
        Some(&pod_iri)
    ).unwrap();

    // Test text search
    let results = graph.search_content("test", Some(10)).unwrap();
    let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();

    // Should find results containing "test"
    assert!(parsed_results.get("results").is_some());
    let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
    assert!(bindings.len() > 0);

    // Test case-insensitive search
    let results = graph.search_content("TEST", Some(10)).unwrap();
    let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();
    let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
    assert!(bindings.len() > 0);

    // Test search with no results
    let results = graph.search_content("nonexistent", Some(10)).unwrap();
    let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();
    let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 0);
}

#[test]
fn test_search_by_type() {
    let (graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod";
    let pod_iri = format!("ant://{}", pod_address);

    // Add test data with types
    graph.put_quad(
        "ant://subject1",
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
        "http://schema.org/MediaObject",
        Some(&pod_iri)
    ).unwrap();

    graph.put_quad(
        "ant://subject2",
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
        "http://schema.org/Person",
        Some(&pod_iri)
    ).unwrap();

    // Test type search
    let results = graph.search_by_type("http://schema.org/MediaObject", Some(10)).unwrap();
    let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();

    let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 1);

    // Verify the correct subject was found
    let subject_value = bindings[0]["subject"]["value"].as_str().unwrap();
    assert_eq!(subject_value, "ant://subject1");
}

#[test]
fn test_search_by_predicate() {
    let (graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod";
    let pod_iri = format!("ant://{}", pod_address);

    // Add test data with specific predicates
    graph.put_quad(
        "ant://subject1",
        "ant://colonylib/vocabulary/0.1/predicate#name",
        "Test Name",
        Some(&pod_iri)
    ).unwrap();

    graph.put_quad(
        "ant://subject2",
        "ant://colonylib/vocabulary/0.1/predicate#description",
        "Test Description",
        Some(&pod_iri)
    ).unwrap();

    // Test predicate search
    let results = graph.search_by_predicate(
        "ant://colonylib/vocabulary/0.1/predicate#name",
        Some(10)
    ).unwrap();
    let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();

    let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 1);

    // Verify the correct subject and object were found
    let subject_value = bindings[0]["subject"]["value"].as_str().unwrap();
    let object_value = bindings[0]["object"]["value"].as_str().unwrap();
    assert_eq!(subject_value, "ant://subject1");
    assert_eq!(object_value, "Test Name");
}

#[test]
fn test_advanced_search() {
    let (graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod";
    let pod_iri = format!("ant://{}", pod_address);

    // Add comprehensive test data
    graph.put_quad(
        "ant://subject1",
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
        "http://schema.org/MediaObject",
        Some(&pod_iri)
    ).unwrap();

    graph.put_quad(
        "ant://subject1",
        "ant://colonylib/vocabulary/0.1/predicate#name",
        "Test Media File",
        Some(&pod_iri)
    ).unwrap();

    let query = r#"
        SELECT DISTINCT ?subject ?predicate ?object ?graph WHERE {{
            GRAPH ?graph {{
                ?subject ?predicate ?object .
                FILTER(isLiteral(?object) && CONTAINS(LCASE(STR(?object)), LCASE("media")))
            }}
        }}
        ORDER BY ?graph ?subject
        LIMIT 10
    "#;
    let results = graph.advanced_search(query).unwrap();
    let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();

    let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
    assert!(bindings.len() > 0);

}

#[test]
fn test_get_pod_scratchpads() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod_123";
    let scratchpad1 = "scratchpad_addr_1";
    let scratchpad2 = "scratchpad_addr_2";
    let configuration_address = "test_config_123";
    let configuration_scratchpad_address = "test_config_scratchpad_123";

    // First create a pod entry to establish the named graph
    graph.add_pod_entry("Test Pod", pod_address, scratchpad1, configuration_address, configuration_scratchpad_address, 0).unwrap();

    // Add additional scratchpad to the pod
    let pod_iri = format!("ant://{}", pod_address);
    let scratchpad2_iri = format!("ant://{}", scratchpad2);
    graph.put_quad(&scratchpad2_iri, HAS_POD_INDEX, "1", Some(&pod_iri)).unwrap();

    // Test getting scratchpads for the pod
    let scratchpads = graph.get_pod_scratchpads(pod_address).unwrap();
    assert_eq!(scratchpads.len(), 2, "Should have 2 scratchpads");
    assert!(scratchpads.contains(&scratchpad1.to_string()));
    assert!(scratchpads.contains(&scratchpad2.to_string()));

    // Test getting scratchpads for non-existent pod
    let empty_scratchpads = graph.get_pod_scratchpads("non_existent_pod").unwrap();
    assert_eq!(empty_scratchpads.len(), 0, "Should have 0 scratchpads for non-existent pod");
}

#[test]
fn test_clear_pod_graph() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod_clear";
    let scratchpad_address = "test_scratchpad_clear";
    let configuration_address = "test_config_clear";
    let configuration_scratchpad_address = "test_config_scratchpad_clear";

    // Add a pod entry
    let result = graph.add_pod_entry("Test Pod for Clearing", pod_address, scratchpad_address, configuration_address, configuration_scratchpad_address, 0);
    assert!(result.is_ok(), "Failed to add pod entry");

    // Verify the pod has data
    let scratchpads_before = graph.get_pod_scratchpads(pod_address).unwrap();
    assert_eq!(scratchpads_before.len(), 1, "Should have 1 scratchpad before clearing");

    // Clear the pod graph
    let clear_result = graph.clear_pod_graph(pod_address);
    assert!(clear_result.is_ok(), "Failed to clear pod graph: {:?}", clear_result.err());

    // Verify the pod graph is cleared
    let scratchpads_after = graph.get_pod_scratchpads(pod_address).unwrap();
    assert_eq!(scratchpads_after.len(), 0, "Should have 0 scratchpads after clearing");
}

#[test]
fn test_load_pod_into_graph() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod_load";

    // Create some test TriG data
    let trig_data = format!(r#"
            <ant://test_subject> <ant://test_predicate> "test_object" .
            <ant://scratchpad123> <{}> "0" .
    "#, HAS_POD_INDEX);

    // Load the data into the graph
    let result = graph.load_pod_into_graph(pod_address, &trig_data);
    assert!(result.is_ok(), "Failed to load pod into graph: {:?}", result.err());

    // Verify the data was loaded by checking for scratchpads
    let scratchpads = graph.get_pod_scratchpads(pod_address).unwrap();
    assert_eq!(scratchpads.len(), 1, "Should have 1 scratchpad after loading");
    assert!(scratchpads.contains(&"scratchpad123".to_string()));
}

#[test]
fn test_get_pod_references_with_add_ref() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod_refs";
    let ref_pod1 = "referenced_pod_1";
    let ref_pod2 = "referenced_pod_2";
    let configuration_address = "test_config_refs";
    let configuration_scratchpad_address = "test_config_scratchpad_refs";

    // Create a pod entry first
    graph.add_pod_entry("Test Pod with References", pod_address, "scratchpad_main", configuration_address, configuration_scratchpad_address, 0).unwrap();

    // Add pod references
    graph.pod_ref_entry(pod_address, ref_pod1, configuration_address, true).unwrap();
    graph.pod_ref_entry(pod_address, ref_pod2, configuration_address, true).unwrap();

    // Test getting pod references
    let references = graph.get_pod_references(pod_address).unwrap();
    assert_eq!(references.len(), 2, "Should have 2 pod references");
    assert!(references.contains(&ref_pod1.to_string()));
    assert!(references.contains(&ref_pod2.to_string()));

    // Test getting references for pod with no references
    let empty_refs = graph.get_pod_references("pod_with_no_refs").unwrap();
    assert_eq!(empty_refs.len(), 0, "Should have 0 references for pod with no references");
}

#[test]
fn test_enhanced_search_content() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod_search";

    // Add a pod with some searchable content
    graph.add_pod_entry("Searchable Pod", pod_address, "search_scratchpad", "search_config", "search_config_scratchpad", 0).unwrap();

    // Add some content to search for
    let pod_iri = format!("ant://{}", pod_address);
    graph.put_quad("ant://test_file", "ant://name", "important_document.pdf", Some(&pod_iri)).unwrap();
    graph.put_quad("ant://test_file", "ant://description", "This is a very important document", Some(&pod_iri)).unwrap();

    // Search for content
    let search_results = graph.search_content("important", Some(10)).unwrap();
    assert!(!search_results.is_empty(), "Search results should not be empty");

    // Parse the JSON to verify it contains our data
    let json_result: serde_json::Value = serde_json::from_str(&search_results).unwrap();
    let bindings = json_result["results"]["bindings"].as_array().unwrap();
    assert!(bindings.len() > 0, "Should have at least one search result");

    // Verify the search found our content
    let found_important = bindings.iter().any(|binding| {
        binding["object"]["value"].as_str().unwrap_or("").contains("important")
    });
    assert!(found_important, "Search should find content containing 'important'");
}

#[test]
fn test_enhanced_word_based_search() {
    let (mut graph, _temp_dir) = create_test_graph();

    // Create pods with different depths
    let pod1_address = "test_pod_depth_0";
    let pod2_address = "test_pod_depth_1";
    let pod3_address = "test_pod_depth_2";

    // Add pods with different depths
    graph.add_pod_entry("Pod at Depth 0", pod1_address, "scratchpad1", "config1", "config_scratchpad1", 0).unwrap();
    graph.add_pod_entry("Pod at Depth 1", pod2_address, "scratchpad2", "config2", "config_scratchpad2", 0).unwrap();
    graph.add_pod_entry("Pod at Depth 2", pod3_address, "scratchpad3", "config3", "config_scratchpad3", 0).unwrap();

    // Set different depths for the pods (using force_set for testing)
    graph.force_set_pod_depth(pod1_address, "config1", 0).unwrap();
    graph.force_set_pod_depth(pod2_address, "config2", 1).unwrap();
    graph.force_set_pod_depth(pod3_address, "config3", 2).unwrap();

    // Verify that depths were set correctly
    assert_eq!(graph.get_pod_depth(pod1_address).unwrap(), 0);
    assert_eq!(graph.get_pod_depth(pod2_address).unwrap(), 1);
    assert_eq!(graph.get_pod_depth(pod3_address).unwrap(), 2);

    // Add content with varying match counts
    let pod1_iri = format!("ant://{}", pod1_address);
    let pod2_iri = format!("ant://{}", pod2_address);
    let pod3_iri = format!("ant://{}", pod3_address);

    // Pod 1 (depth 0): Contains "beatles" and "abbey" (2 matches)
    graph.put_quad("ant://album1", "ant://title", "The Beatles Abbey Road", Some(&pod1_iri)).unwrap();

    // Pod 2 (depth 1): Contains "beatles", "abbey", and "road" (3 matches)
    graph.put_quad("ant://album2", "ant://description", "The Beatles recorded Abbey Road album", Some(&pod2_iri)).unwrap();

    // Pod 3 (depth 2): Contains only "beatles" (1 match)
    graph.put_quad("ant://album3", "ant://artist", "The Beatles", Some(&pod3_iri)).unwrap();

    // Search for "beatles abbey road" - should return results ordered by match count, then by depth
    let search_results = graph.search_content("beatles abbey road", Some(10)).unwrap();
    assert!(!search_results.is_empty(), "Search results should not be empty");

    // Parse the JSON to verify ordering
    let json_result: serde_json::Value = serde_json::from_str(&search_results).unwrap();
    let bindings = json_result["results"]["bindings"].as_array().unwrap();
    assert!(bindings.len() >= 3, "Should have at least 3 search results");

    // Verify that results are ordered by match count (descending) then by depth (ascending)
    // The result with 3 matches (pod2) should come first
    // Then the result with 2 matches (pod1) should come second
    // Finally the result with 1 match (pod3) should come last



    // Verify that results are ordered by match count (descending) then by depth (ascending)
    // Both pod1 and pod2 have 3 matches, so pod1 (depth 0) should come before pod2 (depth 1)
    // Then pod3 with 1 match (depth 2) should come last

    let first_result_text = bindings[0]["object"]["value"].as_str().unwrap_or("");
    let second_result_text = bindings[1]["object"]["value"].as_str().unwrap_or("");
    let third_result_text = bindings[2]["object"]["value"].as_str().unwrap_or("");

    // First result should be pod1 (depth 0, 3 matches)
    assert!(first_result_text.contains("Abbey Road") && !first_result_text.contains("recorded"),
            "First result should be pod1 with depth 0");

    // Second result should be pod2 (depth 1, 3 matches)
    assert!(second_result_text.contains("recorded"),
            "Second result should be pod2 with depth 1");

    // Third result should be pod3 (depth 2, 1 match)
    assert!(third_result_text == "The Beatles" && !third_result_text.contains("Abbey"),
            "Third result should be pod3 with depth 2");

    // Test single word search
    let single_word_results = graph.search_content("beatles", Some(10)).unwrap();
    let single_json: serde_json::Value = serde_json::from_str(&single_word_results).unwrap();
    let single_bindings = single_json["results"]["bindings"].as_array().unwrap();
    assert!(single_bindings.len() >= 3, "Single word search should find all Beatles references");

    // Test empty search
    let empty_results = graph.search_content("", Some(10)).unwrap();
    assert_eq!(empty_results, "[]", "Empty search should return empty array");

    // Test search with no matches
    let no_match_results = graph.search_content("nonexistent", Some(10)).unwrap();
    let no_match_json: serde_json::Value = serde_json::from_str(&no_match_results).unwrap();
    let no_match_bindings = no_match_json["results"]["bindings"].as_array().unwrap();
    assert_eq!(no_match_bindings.len(), 0, "Search with no matches should return empty results");
}
