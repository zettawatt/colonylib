use ruint::Uint;
use autonomi::{Wallet, Client};
use colonylib::{KeyStore, PodManager, DataStore, Graph};
use tokio;
use tracing::{Level};
use tracing_subscriber::{filter, prelude::*};
use serde_json::{Value, json};

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

    println!("podman {:#?}", podman);
    let data = podman.dump_graph("c859818c623ce4fc0899c2ab43061b19caa0b0598eec35ef309dbe50c8af8d59").await.unwrap();
    println!("Graph data: {}", String::from_utf8(data).unwrap());

    // // refresh local cache
    // podman.refresh_cache().await.unwrap();

    // // Add pod 1
    // let (pointer_address1, scratchpad_address1) = podman.add_pod().await.unwrap();
    // println!("Uploaded pointer address: {}", pointer_address1);
    // println!("Uploaded scratchpad address: {}", scratchpad_address1);

    // // Add pod 2
    // let (pointer_address2, scratchpad_address2) = podman.add_pod().await.unwrap();
    // println!("Uploaded pointer address: {}", pointer_address2);
    // println!("Uploaded scratchpad address: {}", scratchpad_address2);


    // let file_data = json!({
    //     "@context": {"schema": "http://schema.org/"},
    //     "@type": "schema:MediaObject",
    //     "@id": "ant://4467c38f2591ddf840161dfd8536bfff594be4e455bf2630e841de846d49029a",
    //     "schema:name": "ant_girl.png",
    //     "schema:description": "A drawing of an ant girl",
    //     "schema:contentSize": "2MB"
    // });
    // let file_data = serde_json::to_string(&file_data).unwrap();
    // let file_data = file_data.as_str();
    // println!("Adding file data: {}", file_data);
    // let _ = podman.put_subject_data(
    //     pointer_address1.trim(),
    //     "4467c38f2591ddf840161dfd8536bfff594be4e455bf2630e841de846d49029a",
    //     file_data).await.unwrap();

    // // Add beg blag info
    // let file_data = json!({
    //     "@context": {"schema": "http://schema.org/"},
    //     "@type": "schema:MediaObject",
    //     "@id": "ant://c859818c623ce4fc0899c2ab43061b19caa0b0598eec35ef309dbe50c8af8d59",
    //     "schema:name": "BegBlag.mp3",
    //     "schema:description": "Beg Blag and Steal STUFF",
    //     "schema:contentSize": "4MB"
    // });
    // let file_data = serde_json::to_string(&file_data).unwrap();
    // let file_data = file_data.as_str();
    // let _ = podman.put_subject_data(
    // pointer_address2.trim(),
    // "c859818c623ce4fc0899c2ab43061b19caa0b0598eec35ef309dbe50c8af8d59",
    // file_data).await.unwrap();

    // // Upload pods
    // let _ = podman.upload_all().await.unwrap();
    // println!("Uploaded all pods");

    // // // wait for the pod to be replicated
    // tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Get the beg blag data
    // let subject_data = podman.get_subject_data(
    //     "c859818c623ce4fc0899c2ab43061b19caa0b0598eec35ef309dbe50c8af8d59").await.unwrap();
    let subject_data = podman.get_subject_data(
        "5467c38f2591ddf840161dfd8536bfff594be4e455bf2630e841de846d49029a").await.unwrap();


    // // Update pod 1
    // let data = "testing update";
    // let _ = podman.update_pod(pointer_address1.trim(), data).unwrap();
    // println!("Updated pod address: {}", pointer_address1);

    // // Add pod 3
    // let data = "testing3";
    // let (pointer_address3, scratchpad_address3) = podman.add_pod().await.unwrap();
    // let _ = podman.update_pod(pointer_address3.trim(), data).unwrap();
    // println!("Uploaded pointer address: {}", pointer_address3);
    // println!("Uploaded scratchpad address: {}", scratchpad_address3);
    // println!("Added pod with data: {}", data);

    // // Upload pods
    // let _ = podman.upload_all().await.unwrap();
    // println!("Uploaded all pods");

    // // wait for the pod to be replicated
    // tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

//    let pod_data = podman.download(pointer_address.to_string()).await.unwrap();
//    println!("Retrieved pod data: {}", pod_data);
//
//    // Update pod 1
//    let data = "updated1";
//    let _ = podman.upload(pointer_address.to_string(), data).await.unwrap();
//    println!("Updated pod with data: {}", data);
//
//    // wait for the pod to be replicated
//    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
//
//    let pod_data = podman.download(pointer_address.to_string()).await.unwrap();
//    println!("Retrieved pod data: {}", pod_data);
//
//    // Add pod 2
//    let data = "testing2";
//    let (pointer_address, scratchpad_address) = podman.create(data).await.unwrap();
//    println!("Uploaded pointer address: {}", pointer_address);
//    println!("Uploaded scratchpad address: {}", scratchpad_address);
//    println!("Added pod with data: {}", data);
//
//    // wait for the pod to be replicated
//    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
//
//    let pod_data = podman.download(pointer_address.to_string()).await.unwrap();
//    println!("Retrieved pod data: {}", pod_data);
//
//    // Update pod 2
//    let data = "updated2";
//    let _ = podman.upload(pointer_address.to_string(), data).await.unwrap();
//    println!("Updated pod with data: {}", data);
//
//    // wait for the pod to be replicated
//    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
//
//    let pod_data = podman.download(pointer_address.to_string()).await.unwrap();
//    println!("Retrieved pod data: {}", pod_data);

// let key_store_file = podman.data_store.get_keystore_path();
// let mut file = std::fs::File::create(key_store_file).unwrap();
// let _ = KeyStore::to_file(key_store, &mut file, PASSWORD).unwrap();

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
