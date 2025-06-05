use ruint::Uint;
use autonomi::{Wallet, Client};
use colonylib::{KeyStore, PodManager, DataStore, Graph};
use tokio;
use tracing::{Level};
use tracing_subscriber::{filter, prelude::*};
use serde_json::json;

// ETH wallet for local testnet
const LOCAL_PRIVATE_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const PASSWORD: &str = "password";

#[tokio::main]
async fn main() {

    let subscriber = tracing_subscriber::registry()
        .with(filter::Targets::new()
            .with_target("colonylib", Level::DEBUG) // INFO level for colonylib
            .with_target("main", Level::INFO)      // INFO level for main.rs
            .with_default(Level::ERROR))          // ERROR level for other modules
        .with(tracing_subscriber::fmt::layer());

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    println!("Connecting to network...");
    let environment = "local".to_string();
    let client = init_client(environment).await;
    let evm_network = client.evm_network();
    let wallet = &Wallet::new_from_private_key(evm_network.clone(), LOCAL_PRIVATE_KEY).unwrap();

    // Use existing setup (assumes setup.rs has been run)
    let data_store = &mut DataStore::create().unwrap();
    let key_store_file = data_store.get_keystore_path();
    let key_store: &mut KeyStore = if key_store_file.exists() {
        println!("Loading existing key store...");
        let mut file = std::fs::File::open(key_store_file).unwrap();
        &mut KeyStore::from_file(&mut file, PASSWORD).unwrap()
    } else {
        println!("Key store file does not exist, creating new key store");
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        &mut KeyStore::from_mnemonic(mnemonic).unwrap()
    };
    let _ = key_store.set_wallet_key(LOCAL_PRIVATE_KEY.to_string()).unwrap();

    let graph_path = data_store.get_graph_path();
    let graph = &mut Graph::open(&graph_path).unwrap();

    let mut podman = PodManager::new(client, wallet, data_store, key_store, graph).await.unwrap();
    println!("Connected to network");

    println!("\n=== Refreshing All Known Pods ===");
    podman.refresh_ref(3).await.unwrap();

    println!("\n=== Colony Search Examples ===");

    // Example 1: Simple text search
    println!("=== Simple Text Search ===");
    let res = podman.search(json!("beg")).await.unwrap();
    println!("Text search results: {}", serde_json::to_string_pretty(&res).unwrap());

    // Example 2: Search for media objects by type
    println!("\n=== Search by Type ===");
    let res = podman.search(json!({
        "type": "by_type",
        "type_uri": "http://schema.org/MediaObject",
        "limit": 10
    })).await.unwrap();
    println!("Type search results: {}", serde_json::to_string_pretty(&res).unwrap());

    // Example 3: Search for files with "name" property
    println!("\n=== Search by Predicate ===");
    let res = podman.search(json!({
        "type": "by_predicate",
        "predicate_uri": "http://schema.org/name",
        "limit": 10
    })).await.unwrap();
    println!("Predicate search results: {}", serde_json::to_string_pretty(&res).unwrap());

    // Example 4: Advanced search with multiple criteria
    println!("\n=== Advanced Search ===");
    let res = podman.search(json!({
        "type": "advanced",
        "text": "drawing",
        "type": "http://schema.org/MediaObject",
        "limit": 5
    })).await.unwrap();
    println!("Advanced search results: {}", serde_json::to_string_pretty(&res).unwrap());

    // Example 5: Search for specific content
    println!("\n=== Content Search ===");
    let res = podman.search(json!({
        "type": "text",
        "text": "beg blag",
        "limit": 10
    })).await.unwrap();
    println!("Content search results: {}", serde_json::to_string_pretty(&res).unwrap());

    // Example 6: Retrieve specific subject data
    println!("\n=== Subject Data Retrieval ===");
    match podman.get_subject_data("5467c38f2591ddf840161dfd8536bfff594be4e455bf2630e841de846d49029a").await {
        Ok(subject_data) => {
            println!("Retrieved subject data: {}", serde_json::to_string_pretty(&subject_data).unwrap());
        }
        Err(e) => {
            println!("Failed to retrieve subject data: {}", e);
        }
    }


    println!("\n=== Search Examples Complete ===");
    println!("Note: Run setup.rs first to initialize the data directory,");
    println!("then run add_pods.rs to create pods with sample data before running this example.");
}

// Get balance of gas tokens in wallet
#[allow(dead_code)]
async fn get_balance_of_gas_tokens (wallet: &Wallet) -> Result<f64, String> {
    let balance: Uint<256, 4> = wallet.balance_of_gas_tokens().await.map_err(|e| {
            println!("Error getting balance of gas tokens: {e}");
            format!("Error getting balance of gas tokens: {e}")
        })?;
    let balance: f64 = balance.try_into().unwrap_or(0f64);
    let balance: f64 = balance / 1_000_000_000_000_000_000.0f64;
    Ok(balance)
}
// Get balance of ANT tokens in wallet
#[allow(dead_code)]
async fn get_balance_of_tokens (wallet: &Wallet) -> Result<f64, String> {
    let balance: Uint<256, 4> = wallet.balance_of_tokens().await.map_err(|e| {
            println!("Error getting balance of gas tokens: {e}");
            format!("Error getting balance of gas tokens: {e}")
        })?;
    let balance: f64 = balance.try_into().unwrap_or(0f64);
    let balance: f64 = balance / 1_000_000_000_000_000_000.0f64;
    Ok(balance)
}


async fn init_client(environment: String) -> Client {
    match environment.trim() {
        "local" => Client::init_local().await.unwrap(),
        "alpha" => Client::init_alpha().await.unwrap(),
        _ => Client::init().await.unwrap(), // "autonomi"
    }
}
