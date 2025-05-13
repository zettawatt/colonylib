use autonomi::{Client, Wallet, SecretKey, PublicKey};
use ruint::Uint;

use crate::KeyStore;

#[derive(Clone)]
pub struct Network {
    client: Client,
    wallet: Wallet,
}

impl Network {

    /// Initialize the client and wallet
    pub async fn initialize(mut wallet_key: String, environment: String) -> Result<Self, String> {
        println!("Initializing client with environment: {environment:?}");

        let client = init_client(environment).await?;
        println!("Client initialized");

        let evm_network = client.evm_network();
        println!("EVM network: {evm_network:?}");

        let wallet =
            Wallet::new_from_private_key(evm_network.clone(), wallet_key.as_str()).map_err(|e| {
                println!("Error loading wallet: {e}");
                format!("Error loading wallet: {e}")
            })?;
        println!("Wallet loaded");

        Ok(Self { wallet, client })
    }

    // Get balance of gas tokens in wallet
    pub async fn get_balance_of_gas_tokens (&self) -> Result<f64, String> {
        let balance: Uint<256, 4> = self.wallet.balance_of_gas_tokens().await.map_err(|e| {
                println!("Error getting balance of gas tokens: {e}");
                format!("Error getting balance of gas tokens: {e}")
            })?;
        let balance: f64 = balance.try_into().unwrap_or(0f64);
        let balance: f64 = balance / 1_000_000_000_000_000_000.0f64;
        Ok(balance)
    }

    // Get balance of ANT tokens in wallet
    pub async fn get_balance_of_tokens (&self) -> Result<f64, String> {
        let balance: Uint<256, 4> = self.wallet.balance_of_tokens().await.map_err(|e| {
                println!("Error getting balance of gas tokens: {e}");
                format!("Error getting balance of gas tokens: {e}")
            })?;
        let balance: f64 = balance.try_into().unwrap_or(0f64);
        let balance: f64 = balance / 1_000_000_000_000_000_000.0f64;
        Ok(balance)
    }

    // Add a new pod
    pub async fn add_pod(&mut self, mut key_store: KeyStore) -> Result<String, String> {
        // Derive a new key for the pod pointer
        let pointer_key = key_store.add_derived_key();

        // Check that this address doesn't contain existing data, else create new key

        // Derive a new key for the pod scratchpad
        let scratchpad_key = key_store.add_derived_key();
        
        // Check that this address doesn't contain existing data, else create new key

        // Create new scratchpad

        // Create new pointer that points to the scratchpad

        // Get quote to upload pointer and scratchpad

        // Pay and upload the scratchpad and pointer to the network

        // Update key store with latest data

        Ok(pointer_key) //FIXME: change this to the address of the pointer
    }

    // Update pod
    pub async fn update_pod(&mut self, address: String, key_store: KeyStore) -> Result<(), String> {
        // Lookup the key for the pod pointer from the key store
        let pointer_key = key_store.get_pod_key(address.clone());

        // Get the pointer value to get the scratchpad address

        // Lookup the scratchpad key from the returned pointer value

        

        // Pay and update the pod on the network

        Ok(()) //FIXME: need a return value for a success??
    }

    // Refresh pod cache
    pub async fn refresh_pod_cache(self, key_store: KeyStore) -> Result<(), String> {
        // Get the list of pods from the key store

        // Go through each pointer and check if there is an update vs the cache

        // If the pointer is newer, download and update the associated scratchpad and set the depth attribute

        // Recurse through each of the pods listed in the scratchpad and perform the same operation, increasing the depth attribute

        Ok(()) //FIXME: need a return value for a success??
    }


}

async fn init_client(environment: String) -> Result<Client, String> {
    let res = match environment.as_str() {
        "local" => Client::init_local().await,
        "alpha" => Client::init_alpha().await,
        _ => Client::init().await, // "autonomi"
    };
    res.map_err(|e| {
        println!("Error initializing client: {e}");
        format!("Error initializing client: {e}")
    })
}