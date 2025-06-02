use colonylib::{DataStore, KeyStore, Graph};
use tempfile::TempDir;

/// Helper function to create test components for integration tests
#[allow(dead_code)]
pub fn create_test_components() -> (DataStore, KeyStore, Graph, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let data_dir = temp_dir.path().join("data");
    let pods_dir = temp_dir.path().join("pods");
    let pod_refs_dir = temp_dir.path().join("pod_refs");
    let downloads_dir = temp_dir.path().join("downloads");

    let data_store = DataStore::from_paths(data_dir.clone(), pods_dir, pod_refs_dir, downloads_dir)
        .expect("Failed to create test datastore");

    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let key_store = KeyStore::from_mnemonic(mnemonic).expect("Failed to create keystore");

    let graph_path = data_dir.join("test_graph.db");
    let graph = Graph::open(&graph_path).expect("Failed to create graph");

    (data_store, key_store, graph, temp_dir)
}

/// Helper function to create a test DataStore
#[allow(dead_code)]
pub fn create_test_datastore() -> (DataStore, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let data_dir = temp_dir.path().join("data");
    let pods_dir = temp_dir.path().join("pods");
    let pod_refs_dir = temp_dir.path().join("pod_refs");
    let downloads_dir = temp_dir.path().join("downloads");

    let datastore = DataStore::from_paths(data_dir, pods_dir, pod_refs_dir, downloads_dir)
        .expect("Failed to create test datastore");
    (datastore, temp_dir)
}

/// Helper function to create a test Graph
#[allow(dead_code)]
pub fn create_test_graph() -> (Graph, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let db_path = temp_dir.path().join("test_graph.db");
    let graph = Graph::open(&db_path).expect("Failed to create test graph");
    (graph, temp_dir)
}
