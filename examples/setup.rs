use ruint::Uint;
use autonomi::{Wallet, Client};
use colonylib::{KeyStore, PodManager, DataStore, Graph};
use tokio;
use tracing::{Level};
use tracing_subscriber::{filter, prelude::*};

// ETH wallet for local testnet
const LOCAL_PRIVATE_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
//const LOCAL_ADDRESS: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
//const ALPHA_PRIVATE_KEY: &str = "d1d4b50cc66326a8f6ce00be7a7f4682ecd5056b911b98719cec06a32c64330b";
//const ALPHA_ADDRESS: &str = "0xA43AbD0FFDB53AA3f03A6BE079ACbB5635400444";
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

    println!("Creating network");
    let environment = "local".to_string();
    //let environment = "alpha".to_string();
    println!("Initializing client with environment: {environment:?}");
    let client = init_client(environment).await;
    println!("Client initialized");
    let evm_network = client.evm_network();
    println!("EVM network: {evm_network:?}");
    //FIXME: need to grap the wallet error and remove this unwrap()
    let wallet = &Wallet::new_from_private_key(evm_network.clone(), LOCAL_PRIVATE_KEY).unwrap();
    println!("Wallet loaded");

    // Clean and reinitialize the default data directory
    let data_store = &mut DataStore::create().unwrap();

    let key_store_file = data_store.get_keystore_path();
    let key_store: &mut KeyStore = if key_store_file.exists() {
        println!("Key store file already exists, loading from file");
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
    println!("Network created");
    let balance = get_balance_of_tokens(wallet).await.unwrap();
    println!("Balance of tokens: {}", balance);
    let balance = get_balance_of_gas_tokens(wallet).await.unwrap();
    println!("Balance of gas tokens: {}", balance);

    println!("\n=== Refreshing All Known Pods ===");
    podman.refresh_ref(3).await.unwrap();

    let key_store_file = podman.data_store.get_keystore_path();
    let mut file = std::fs::File::create(key_store_file).unwrap();
    let _ = KeyStore::to_file(key_store, &mut file, PASSWORD).unwrap();

    println!("Setup complete! Data directory cleaned and reinitialized.");
}

// Get balance of gas tokens in wallet
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
