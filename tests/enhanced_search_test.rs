mod common;
use common::create_test_graph;

#[test]
fn test_enhanced_word_based_search() {
    let (mut graph, _temp_dir) = create_test_graph();

    // Create pods with different depths
    let pod1_address = "test_pod_depth_0";
    let pod2_address = "test_pod_depth_1";
    let pod3_address = "test_pod_depth_2";

    // Add pods with different depths
    graph
        .add_pod_entry(
            "Pod at Depth 0",
            pod1_address,
            "scratchpad1",
            "config1",
            "config_scratchpad1",
            0,
        )
        .unwrap();
    graph
        .add_pod_entry(
            "Pod at Depth 1",
            pod2_address,
            "scratchpad2",
            "config2",
            "config_scratchpad2",
            0,
        )
        .unwrap();
    graph
        .add_pod_entry(
            "Pod at Depth 2",
            pod3_address,
            "scratchpad3",
            "config3",
            "config_scratchpad3",
            0,
        )
        .unwrap();

    // Set different depths for the pods
    graph.update_pod_depth(pod1_address, "config1", 0).unwrap();
    graph.update_pod_depth(pod2_address, "config2", 1).unwrap();
    graph.update_pod_depth(pod3_address, "config3", 2).unwrap();

    // Add content with varying match counts
    let pod1_iri = format!("ant://{}", pod1_address);
    let pod2_iri = format!("ant://{}", pod2_address);
    let pod3_iri = format!("ant://{}", pod3_address);

    // Pod 1 (depth 0): Contains "beatles" and "abbey" (2 matches)
    graph
        .put_quad(
            "ant://album1",
            "ant://title",
            "The Beatles Abbey Road",
            Some(&pod1_iri),
        )
        .unwrap();

    // Pod 2 (depth 1): Contains "beatles", "abbey", and "road" (3 matches)
    graph
        .put_quad(
            "ant://album2",
            "ant://description",
            "The Beatles recorded Abbey Road album",
            Some(&pod2_iri),
        )
        .unwrap();

    // Pod 3 (depth 2): Contains only "beatles" (1 match)
    graph
        .put_quad(
            "ant://album3",
            "ant://artist",
            "The Beatles",
            Some(&pod3_iri),
        )
        .unwrap();

    // Search for "beatles abbey road" - should return results ordered by match count, then by depth
    let search_results = graph
        .search_content("beatles abbey road", Some(10))
        .unwrap();
    assert!(
        !search_results.is_empty(),
        "Search results should not be empty"
    );

    // Parse the JSON to verify ordering
    let json_result: serde_json::Value = serde_json::from_str(&search_results).unwrap();
    let bindings = json_result["results"]["bindings"].as_array().unwrap();
    assert!(bindings.len() >= 3, "Should have at least 3 search results");

    // Verify that results are ordered by match count (descending) then by depth (ascending)
    // Both "The Beatles Abbey Road" and "The Beatles recorded Abbey Road album" have 3 matches
    // But "The Beatles recorded Abbey Road album" should come first if it has lower depth
    // "The Beatles" has only 1 match so should come last

    // Check that results with more matches come before results with fewer matches
    let first_match_count: i32 = bindings[0]
        .get("match_count")
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let last_match_count: i32 = bindings[bindings.len() - 1]
        .get("match_count")
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    assert!(
        first_match_count >= last_match_count,
        "Results should be ordered by match count (descending): first={}, last={}",
        first_match_count,
        last_match_count
    );

    // Test single word search
    let single_word_results = graph.search_content("beatles", Some(10)).unwrap();
    let single_json: serde_json::Value = serde_json::from_str(&single_word_results).unwrap();
    let single_bindings = single_json["results"]["bindings"].as_array().unwrap();
    assert!(
        single_bindings.len() >= 3,
        "Single word search should find all Beatles references"
    );

    // Test empty search
    let empty_results = graph.search_content("", Some(10)).unwrap();
    assert_eq!(
        empty_results, "[]",
        "Empty search should return empty array"
    );

    // Test search with no matches
    let no_match_results = graph.search_content("nonexistent", Some(10)).unwrap();
    let no_match_json: serde_json::Value = serde_json::from_str(&no_match_results).unwrap();
    let no_match_bindings = no_match_json["results"]["bindings"].as_array().unwrap();
    assert_eq!(
        no_match_bindings.len(),
        0,
        "Search with no matches should return empty results"
    );
}

#[test]
fn test_word_splitting_and_or_logic() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod_or_logic";
    graph
        .add_pod_entry(
            "Test Pod",
            pod_address,
            "scratchpad",
            "config",
            "config_scratchpad",
            0,
        )
        .unwrap();

    let pod_iri = format!("ant://{}", pod_address);

    // Add content that matches different combinations of words
    graph
        .put_quad(
            "ant://doc1",
            "ant://title",
            "The Beatles are great",
            Some(&pod_iri),
        )
        .unwrap();
    graph
        .put_quad(
            "ant://doc2",
            "ant://title",
            "Abbey Road is an album",
            Some(&pod_iri),
        )
        .unwrap();
    graph
        .put_quad(
            "ant://doc3",
            "ant://title",
            "Road trip to nowhere",
            Some(&pod_iri),
        )
        .unwrap();

    // Search for "beatles abbey road" should find all three documents
    // doc1 matches "beatles", doc2 matches "abbey" and "road", doc3 matches "road"
    let search_results = graph
        .search_content("beatles abbey road", Some(10))
        .unwrap();
    let json_result: serde_json::Value = serde_json::from_str(&search_results).unwrap();
    let bindings = json_result["results"]["bindings"].as_array().unwrap();

    assert_eq!(
        bindings.len(),
        3,
        "Should find all three documents with OR logic"
    );

    // Verify that doc2 comes first (2 matches), then doc1 and doc3 (1 match each)
    let first_result = bindings[0]["object"]["value"].as_str().unwrap_or("");
    assert!(
        first_result.contains("Abbey Road"),
        "First result should have most matches"
    );
}

#[test]
fn test_quoted_phrase_search() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod_quotes";
    graph
        .add_pod_entry(
            "Test Pod",
            pod_address,
            "scratchpad",
            "config",
            "config_scratchpad",
            0,
        )
        .unwrap();

    let pod_iri = format!("ant://{}", pod_address);

    // Add content with exact phrases and individual words
    graph
        .put_quad(
            "ant://doc1",
            "ant://title",
            "The Beatles Abbey Road album",
            Some(&pod_iri),
        )
        .unwrap();
    graph
        .put_quad(
            "ant://doc2",
            "ant://title",
            "Abbey Road is great",
            Some(&pod_iri),
        )
        .unwrap();
    graph
        .put_quad(
            "ant://doc3",
            "ant://title",
            "The Beatles recorded many albums",
            Some(&pod_iri),
        )
        .unwrap();
    graph
        .put_quad(
            "ant://doc4",
            "ant://title",
            "Road trip with the band",
            Some(&pod_iri),
        )
        .unwrap();

    // Search for 'the beatles "abbey road"' should find:
    // - doc1: matches "the", "beatles", AND "abbey road" phrase (3 matches)
    // - doc2: matches "abbey road" phrase (1 match)
    // - doc3: matches "the" and "beatles" (2 matches)
    // - doc4: matches "the" (1 match)
    let search_results = graph
        .search_content(r#"the beatles "abbey road""#, Some(10))
        .unwrap();
    let json_result: serde_json::Value = serde_json::from_str(&search_results).unwrap();
    let bindings = json_result["results"]["bindings"].as_array().unwrap();

    // Should find all 4 documents since they all contain at least one search term
    assert_eq!(bindings.len(), 4, "Should find exactly 4 documents");

    // Check that doc1 comes first (should have 3 matches)
    let first_result = bindings[0]["object"]["value"].as_str().unwrap_or("");
    assert!(
        first_result.contains("The Beatles Abbey Road"),
        "First result should be the one with most matches"
    );

    // Verify that all results contain at least one of the search terms
    for binding in bindings {
        let text = binding["object"]["value"]
            .as_str()
            .unwrap_or("")
            .to_lowercase();
        let has_the = text.contains("the");
        let has_beatles = text.contains("beatles");
        let has_abbey_road_phrase = text.contains("abbey road");

        assert!(
            has_the || has_beatles || has_abbey_road_phrase,
            "Result '{}' should contain at least one search term",
            text
        );
    }
}

#[test]
fn test_multiple_quoted_phrases() {
    let (mut graph, _temp_dir) = create_test_graph();

    let pod_address = "test_pod_multi_quotes";
    graph
        .add_pod_entry(
            "Test Pod",
            pod_address,
            "scratchpad",
            "config",
            "config_scratchpad",
            0,
        )
        .unwrap();

    let pod_iri = format!("ant://{}", pod_address);

    // Add content to test multiple quoted phrases
    graph
        .put_quad(
            "ant://doc1",
            "ant://title",
            "The Beatles Abbey Road is a great album",
            Some(&pod_iri),
        )
        .unwrap();
    graph
        .put_quad(
            "ant://doc2",
            "ant://title",
            "Led Zeppelin IV is also great",
            Some(&pod_iri),
        )
        .unwrap();
    graph
        .put_quad(
            "ant://doc3",
            "ant://title",
            "Abbey Road by The Beatles",
            Some(&pod_iri),
        )
        .unwrap();

    // Search for '"the beatles" "abbey road"' should find:
    // - doc1: matches both phrases (2 matches)
    // - doc3: matches both phrases (2 matches)
    // - doc2: matches neither phrase (0 matches) - should not appear
    let search_results = graph
        .search_content(r#""the beatles" "abbey road""#, Some(10))
        .unwrap();
    let json_result: serde_json::Value = serde_json::from_str(&search_results).unwrap();
    let bindings = json_result["results"]["bindings"].as_array().unwrap();

    // Should find 2 documents (doc1, doc3) but not doc2
    assert_eq!(
        bindings.len(),
        2,
        "Should find exactly 2 documents with both phrases"
    );

    // Verify that all results contain both phrases
    for binding in bindings {
        let text = binding["object"]["value"]
            .as_str()
            .unwrap_or("")
            .to_lowercase();
        assert!(
            text.contains("the beatles") && text.contains("abbey road"),
            "Result should contain both 'the beatles' and 'abbey road' phrases"
        );
    }
}

#[test]
fn test_search_term_parsing() {
    // Test the parsing function directly by creating a simple test
    let (graph, _temp_dir) = create_test_graph();

    // Test various parsing scenarios by checking the actual search behavior
    let test_cases = vec![
        ("simple words", vec!["simple", "words"]),
        ("\"quoted phrase\"", vec!["quoted phrase"]),
        (
            "word \"quoted phrase\" word",
            vec!["word", "quoted phrase", "word"],
        ),
        (
            "\"first phrase\" \"second phrase\"",
            vec!["first phrase", "second phrase"],
        ),
        (
            "before \"middle phrase\" after",
            vec!["before", "middle phrase", "after"],
        ),
        ("\"unclosed quote", vec!["unclosed quote"]), // Should handle unclosed quotes gracefully
        ("", vec![]),                                 // Empty string
        ("   spaced   words   ", vec!["spaced", "words"]), // Extra whitespace
    ];

    for (input, expected) in test_cases {
        // We can't directly test the private parse_search_terms function,
        // but we can verify the behavior by checking if empty searches return empty results
        if expected.is_empty() {
            let result = graph.search_content(input, Some(1)).unwrap();
            assert_eq!(
                result, "[]",
                "Empty search '{}' should return empty results",
                input
            );
        } else {
            // For non-empty searches, just verify they don't crash and return valid JSON
            let result = graph.search_content(input, Some(1)).unwrap();
            let _: serde_json::Value = serde_json::from_str(&result)
                .unwrap_or_else(|_| panic!("Search '{}' should return valid JSON", input));
        }
    }
}
