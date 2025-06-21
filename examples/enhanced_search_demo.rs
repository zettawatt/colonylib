use colonylib::Graph;
use tempfile::TempDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary graph for demonstration
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("demo_graph.db");
    let mut graph = Graph::open(&db_path)?;

    println!("Enhanced Search Demonstration");
    println!("============================\n");

    // Create some test pods with different depths
    let pod1 = "music_collection_depth_0";
    let pod2 = "music_reviews_depth_1"; 
    let pod3 = "artist_info_depth_2";

    // Add pods with content
    graph.add_pod_entry("Music Collection", pod1, "scratchpad1", "config1", "config_scratchpad1")?;
    graph.add_pod_entry("Music Reviews", pod2, "scratchpad2", "config2", "config_scratchpad2")?;
    graph.add_pod_entry("Artist Info", pod3, "scratchpad3", "config3", "config_scratchpad3")?;

    // Set different depths
    graph.update_pod_depth(pod1, "config1", 0)?;
    graph.update_pod_depth(pod2, "config2", 1)?;
    graph.update_pod_depth(pod3, "config3", 2)?;

    // Add content with varying match patterns
    let pod1_iri = format!("ant://{}", pod1);
    let pod2_iri = format!("ant://{}", pod2);
    let pod3_iri = format!("ant://{}", pod3);

    // Pod 1 (depth 0): Contains "beatles" and "abbey" (2 matches for "beatles abbey road")
    graph.put_quad("ant://album1", "ant://title", "The Beatles Abbey Road", Some(&pod1_iri))?;
    graph.put_quad("ant://album2", "ant://title", "The Beatles White Album", Some(&pod1_iri))?;
    
    // Pod 2 (depth 1): Contains "beatles", "abbey", and "road" (3 matches for "beatles abbey road")
    graph.put_quad("ant://review1", "ant://content", "The Beatles recorded Abbey Road album in 1969", Some(&pod2_iri))?;
    graph.put_quad("ant://review2", "ant://content", "Abbey Road is considered their masterpiece", Some(&pod2_iri))?;
    
    // Pod 3 (depth 2): Contains only "beatles" (1 match for "beatles abbey road")
    graph.put_quad("ant://artist1", "ant://name", "The Beatles", Some(&pod3_iri))?;
    graph.put_quad("ant://artist2", "ant://name", "Led Zeppelin", Some(&pod3_iri))?;

    println!("Added content to pods:");
    println!("- Pod 1 (depth 0): 'The Beatles Abbey Road', 'The Beatles White Album'");
    println!("- Pod 2 (depth 1): 'The Beatles recorded Abbey Road album in 1969', 'Abbey Road is considered their masterpiece'");
    println!("- Pod 3 (depth 2): 'The Beatles', 'Led Zeppelin'");
    println!();

    // Demonstrate the enhanced search
    println!("Search query: 'beatles abbey road'");
    println!("Expected behavior:");
    println!("1. Split into words: ['beatles', 'abbey', 'road']");
    println!("2. Use OR logic: find content containing ANY of these words");
    println!("3. Count matches per result");
    println!("4. Sort by: match count (DESC), then depth (ASC)");
    println!();

    let search_results = graph.search_content("beatles abbey road", Some(10))?;
    let json_result: serde_json::Value = serde_json::from_str(&search_results)?;
    let bindings = json_result["results"]["bindings"].as_array().unwrap();

    println!("Search Results:");
    println!("===============");
    for (i, binding) in bindings.iter().enumerate() {
        let text = binding["object"]["value"].as_str().unwrap_or("");
        let match_count = binding.get("match_count")
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let depth = binding.get("depth")
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        
        println!("{}. '{}' (matches: {}, depth: {})", i + 1, text, match_count, depth);
    }

    println!();
    println!("Notice how:");
    println!("- Results with more word matches appear first");
    println!("- Among results with the same match count, those from lower depth pods appear first");
    println!("- The search finds content containing ANY of the search words (OR logic)");
    println!();

    // Demonstrate quoted phrase search
    println!("Quoted Phrase Search Demo");
    println!("========================");
    println!("Search query: 'beatles \"abbey road\"'");
    println!("Expected behavior:");
    println!("1. Split into terms: ['beatles', 'abbey road']");
    println!("2. Find content containing 'beatles' OR the exact phrase 'abbey road'");
    println!();

    let quoted_search_results = graph.search_content(r#"beatles "abbey road""#, Some(10))?;
    let quoted_json_result: serde_json::Value = serde_json::from_str(&quoted_search_results)?;
    let quoted_bindings = quoted_json_result["results"]["bindings"].as_array().unwrap();

    println!("Quoted Search Results:");
    println!("=====================");
    for (i, binding) in quoted_bindings.iter().enumerate() {
        let text = binding["object"]["value"].as_str().unwrap_or("");
        let match_count = binding.get("match_count")
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let depth = binding.get("depth")
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        println!("{}. '{}' (matches: {}, depth: {})", i + 1, text, match_count, depth);
    }

    println!();
    println!("Notice how:");
    println!("- 'The Beatles Abbey Road' matches both 'beatles' and 'abbey road' phrase (2 matches)");
    println!("- 'The Beatles recorded Abbey Road album' matches both terms (2 matches)");
    println!("- 'Abbey Road is considered...' matches only the 'abbey road' phrase (1 match)");
    println!("- 'The Beatles' matches only 'beatles' (1 match)");
    println!("- Content with just 'road' (not 'abbey road') would not match the quoted phrase");

    Ok(())
}
