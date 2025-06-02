mod common;
use common::create_test_graph;

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

    let result = graph.add_pod_entry(pod_address, scratchpad_address);
    assert!(result.is_ok());

    let trig_data = result.unwrap();
    assert!(!trig_data.is_empty());
    // The function creates a named graph for the pod and adds data about the scratchpad
    assert!(trig_data.contains(&format!("ant://{}", scratchpad_address)));
    // Check for the actual predicate URIs
    assert!(trig_data.contains("colonylib/vocabulary"));
    // Note: depth is stored in the default graph, not in the pod's named graph
    // so it won't appear in the TriG output for the specific pod graph
}

#[test]
fn test_pod_depth_operations() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod_123";

    // Initially, pod should have no depth (returns u64::MAX)
    let initial_depth = graph.get_pod_depth(pod_address).unwrap();
    assert_eq!(initial_depth, u64::MAX);

    // Set depth to 0
    graph.update_pod_depth(pod_address, 0).unwrap();
    let depth = graph.get_pod_depth(pod_address).unwrap();
    assert_eq!(depth, 0);

    // Try to set depth to 2 (should NOT work since 2 > 0, depth should remain 0)
    graph.update_pod_depth(pod_address, 2).unwrap();
    let depth = graph.get_pod_depth(pod_address).unwrap();
    assert_eq!(depth, 0); // Should still be 0 since we only update to smaller depths

    // Try to set depth to 1 (should NOT work since 1 > 0, depth should remain 0)
    graph.update_pod_depth(pod_address, 1).unwrap();
    let depth = graph.get_pod_depth(pod_address).unwrap();
    assert_eq!(depth, 0); // Should still be 0

    // Now let's test with a higher initial depth
    // First set depth to 5
    graph.update_pod_depth(pod_address, 5).unwrap(); // This won't work since 5 > 0
    let depth = graph.get_pod_depth(pod_address).unwrap();
    assert_eq!(depth, 0); // Should still be 0

    // Let's start fresh with a new pod to test the depth logic properly
    let pod_address2 = "test_pod_456";

    // Set initial depth to 5 (this should work since no depth exists)
    graph.update_pod_depth(pod_address2, 5).unwrap();
    let depth = graph.get_pod_depth(pod_address2).unwrap();
    assert_eq!(depth, 5);

    // Try to set depth to 3 (should work since 3 < 5)
    graph.update_pod_depth(pod_address2, 3).unwrap();
    let depth = graph.get_pod_depth(pod_address2).unwrap();
    assert_eq!(depth, 3);

    // Try to set depth to 7 (should not change since 7 > 3)
    graph.update_pod_depth(pod_address2, 7).unwrap();
    let depth = graph.get_pod_depth(pod_address2).unwrap();
    assert_eq!(depth, 3); // Should still be 3
}

#[test]
fn test_get_pods_at_depth() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod1 = "pod1_address";
    let pod2 = "pod2_address";
    let pod3 = "pod3_address";

    // Set different depths
    graph.update_pod_depth(pod1, 0).unwrap();
    graph.update_pod_depth(pod2, 1).unwrap();
    graph.update_pod_depth(pod3, 0).unwrap();

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
fn test_load_trig_data() {
    let (mut graph, _temp_dir) = create_test_graph();

    // Test with empty data
    let result = graph.load_trig_data("");
    assert!(result.is_ok());

    // Test with whitespace only
    let result = graph.load_trig_data("   \n\t  ");
    assert!(result.is_ok());

    // Test with simple TriG data
    let trig_data = r#"
        @prefix ex: <http://example.org/> .
        ex:graph1 {
            ex:subject ex:predicate ex:object .
        }
    "#;

    let result = graph.load_trig_data(trig_data);
    assert!(result.is_ok());
}

#[test]
fn test_get_pod_references() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod";

    // Create a pod with some test data that includes references
    let trig_data = format!(r#"
        @prefix ant: <ant://> .
        <ant://{}> {{
            <ant://subject1> <ant://colonylib/vocabulary/0.1/predicate#references> <ant://referenced_pod1> .
            <ant://subject2> <ant://colonylib/vocabulary/0.1/predicate#references> <ant://referenced_pod2> .
            <ant://subject3> <ant://colonylib/vocabulary/0.1/predicate#name> "Some Name" .
        }}
    "#, pod_address);

    // Load the test data
    graph.load_trig_data(&trig_data).unwrap();

    // Get references
    let references = graph.get_pod_references(pod_address).unwrap();

    // Should find the referenced pods but not vocabulary URIs
    assert!(references.contains(&"ant://referenced_pod1".to_string()));
    assert!(references.contains(&"ant://referenced_pod2".to_string()));

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

    // Test advanced search with text criteria
    let criteria = serde_json::json!({
        "text": "media",
        "limit": 10
    });

    let results = graph.advanced_search(&criteria).unwrap();
    let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();

    let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
    assert!(bindings.len() > 0);

    // Test advanced search with type criteria
    let criteria = serde_json::json!({
        "type": "http://schema.org/MediaObject",
        "limit": 10
    });

    let results = graph.advanced_search(&criteria).unwrap();
    let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();

    let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 1);

    // Test advanced search with predicate criteria
    let criteria = serde_json::json!({
        "predicate": "ant://colonylib/vocabulary/0.1/predicate#name",
        "limit": 10
    });

    let results = graph.advanced_search(&criteria).unwrap();
    let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();

    let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 1);
}
