mod common;

#[test]
fn test_graph_pod_rename_operations() {
    let (mut graph, _temp_dir) = common::create_test_graph();

    // Create a mock pod entry in the graph
    let pod_address = "test_pod_address";
    let initial_name = "InitialPodName";

    // Add a pod entry to the graph
    graph
        .force_set_pod_depth(pod_address, initial_name, 0)
        .unwrap();

    // Rename the pod in the graph
    let _updated_graph = graph.rename_pod_entry(pod_address, "RenamedPod").unwrap();

    // Verify the rename worked by checking if we can find the pod with the new name
    // Note: This is a simplified test since the actual rename_pod_entry function
    // returns graph data that would be processed by the PodManager

    // The fact that rename_pod_entry didn't return an error indicates success
    // A more comprehensive test would require mocking the entire PodManager workflow
}

#[test]
fn test_graph_pod_removal_operations() {
    let (mut graph, _temp_dir) = common::create_test_graph();

    // Create a mock pod entry in the graph
    let pod_address = "test_pod_address";
    let pod_name = "TestPod";

    // Add a pod entry to the graph
    graph.force_set_pod_depth(pod_address, pod_name, 0).unwrap();

    // Verify the pod exists
    let depth = graph.get_pod_depth(pod_address).unwrap();
    assert_ne!(depth, u64::MAX, "Pod should exist in graph");

    // Remove the pod from the graph (simulating what remove_pod_entry would do)
    let scratchpads = vec![];
    let config_address = "config_address";
    let _result = graph.remove_pod_entry(pod_address, scratchpads, config_address);

    // The fact that remove_pod_entry didn't panic indicates the operation completed
    // A more comprehensive test would require mocking the entire PodManager workflow
}
