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

    println!("Setting up network connection...");
    let environment = "local".to_string();
    let client = init_client(environment).await;
    let evm_network = client.evm_network();
    let wallet = &Wallet::new_from_private_key(evm_network.clone(), LOCAL_PRIVATE_KEY).unwrap();

    // Use existing setup from setup.rs
    let data_store = &mut DataStore::create().unwrap();
    let key_store_file = data_store.get_keystore_path();
    let key_store: &mut KeyStore = if key_store_file.exists() {
        println!("Loading existing key store...");
        let mut file = std::fs::File::open(key_store_file).unwrap();
        &mut KeyStore::from_file(&mut file, PASSWORD).unwrap()
    } else {
        println!("Creating new key store...");
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        &mut KeyStore::from_mnemonic(mnemonic).unwrap()
    };
    let _ = key_store.set_wallet_key(LOCAL_PRIVATE_KEY.to_string()).unwrap();

    let graph_path = data_store.get_graph_path();
    let graph = &mut Graph::open(&graph_path).unwrap();

    let mut podman = PodManager::new(client, wallet, data_store, key_store, graph).await.unwrap();
    println!("Network connection established");

    // Add pod 1 with ant girl image metadata
    println!("\n=== Adding Pod 1 ===");
    let (pointer_address1, scratchpad_address1) = podman.add_pod("Pod 1").await.unwrap();
    println!("Pod 1 - Pointer address: {}", pointer_address1);
    println!("Pod 1 - Scratchpad address: {}", scratchpad_address1);

    let file_data1 = json!({
        "@context": {"schema": "http://schema.org/"},
        "@type": "schema:MediaObject",
        "@id": "ant://4467c38f2591ddf840161dfd8536bfff594be4e455bf2630e841de846d49029a",
        "schema:name": "ant_girl.png",
        "schema:description": "A drawing of an ant girl",
        "schema:contentSize": "2MB"
    });
    let file_data1_str = serde_json::to_string(&file_data1).unwrap();
    println!("Adding file data to Pod 1: {}", file_data1_str);
    
    let _ = podman.put_subject_data(
        pointer_address1.trim(),
        "4467c38f2591ddf840161dfd8536bfff594be4e455bf2630e841de846d49029a",
        &file_data1_str).await.unwrap();

    // Add pod 2 with audio file metadata
    println!("\n=== Adding Pod 2 ===");
    let (pointer_address2, scratchpad_address2) = podman.add_pod("Pod 2").await.unwrap();
    println!("Pod 2 - Pointer address: {}", pointer_address2);
    println!("Pod 2 - Scratchpad address: {}", scratchpad_address2);

    let file_data2 = json!({
        "@context": {"schema": "http://schema.org/"},
        "@type": "schema:MediaObject",
        "@id": "ant://c859818c623ce4fc0899c2ab43061b19caa0b0598eec35ef309dbe50c8af8d59",
        "schema:name": "BegBlag.mp3",
        "schema:description": "Beg Blag and Steal",
        "schema:contentSize": "4MB"
    });
    let file_data2_str = serde_json::to_string(&file_data2).unwrap();
    println!("Adding file data to Pod 2: {}", file_data2_str);
    
    let _ = podman.put_subject_data(
        pointer_address2.trim(),
        "c859818c623ce4fc0899c2ab43061b19caa0b0598eec35ef309dbe50c8af8d59",
        &file_data2_str).await.unwrap();

    // Upload both pods to the network
    println!("\n=== Uploading Pods to Network ===");
    let _ = podman.upload_all().await.unwrap();
    println!("Successfully uploaded all pods to the network!");

    // Wait for replication
    println!("Waiting for pod replication...");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Updating pod 2 with an additional file metadata
    println!("\n=== Updating Pod 2 ===");

    let file_data2 = json!({
        "@context": {"schema": "http://schema.org/"},
        "@type": "schema:MediaObject",
        "@id": "ant://01bd818c623ce4fc0899c2ab43061b19caa0b0598eec35ef309dbe50c8af8d59",
        "schema:name": "something.mp3",
        "schema:description": "Some other song",
        "schema:contentSize": "3MB"
    });
    let file_data2_str = serde_json::to_string(&file_data2).unwrap();
    println!("Adding additional file data to Pod 2: {}", file_data2_str);
    
    let _ = podman.put_subject_data(
        pointer_address2.trim(),
        "01bd818c623ce4fc0899c2ab43061b19caa0b0598eec35ef309dbe50c8af8d59",
        &file_data2_str).await.unwrap();

    // Upload both pods to the network
    println!("\n=== Uploading Pods to Network ===");
    let _ = podman.upload_all().await.unwrap();
    println!("Successfully uploaded all pods to the network!");

    // Wait for replication
    println!("Waiting for pod replication...");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    println!("\n=== Summary ===");
    println!("Pod 1 (ant_girl.png): {}", pointer_address1);
    println!("Pod 2 (BegBlag.mp3 and something.mp3): {}", pointer_address2);
    println!("Both pods have been successfully created and uploaded to the network.");
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
