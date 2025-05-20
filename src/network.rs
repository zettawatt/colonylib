use autonomi::{Bytes, Client, PublicKey, SecretKey, Signature, Wallet};
use autonomi::client::pointer::{Pointer, PointerTarget, PointerError, PointerAddress};
use autonomi::client::scratchpad::{Scratchpad, ScratchpadError, ScratchpadAddress};
use autonomi::client::payment::PaymentOption;
use ruint::Uint;
use hex;

use crate::KeyStore;

#[derive(Clone)]
pub struct Network {
    pub client: Client,
    pub wallet: Wallet,
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

    // Create a new pointer key, make sure it is empty, and add it to the key store
    async fn create_pointer_key(&mut self, key_store: &mut KeyStore) -> Result<SecretKey, PointerError> {
        loop {
            // Derive a new key
            let key_string = key_store.add_derived_key();
            let derived_key: SecretKey = SecretKey::from_hex(key_string.as_str()).unwrap();
            // Check if the key is empty
            let address = PointerAddress::new(derived_key.clone().public_key());
            let already_exists = self.client.pointer_check_existance(&address).await?;
            if already_exists {
                println!("Pointer key already exists, generating a new one");
                continue;
            } else {
                return Ok(derived_key);
            }
        }
    }

    // Create a new pointer key, make sure it is empty, and add it to the key store
    async fn create_scratchpad_key(&mut self, key_store: &mut KeyStore) -> Result<SecretKey, ScratchpadError> {
        loop {
            // Derive a new key
            let key_string = key_store.add_derived_key();
            let derived_key: SecretKey = SecretKey::from_hex(key_string.as_str()).unwrap();
            // Check if the key is empty
            let address = ScratchpadAddress::new(derived_key.clone().public_key());
            let already_exists = self.client.scratchpad_check_existance(&address).await?;
            if already_exists {
                println!("Scratchpad key already exists, generating a new one");
                continue;
            } else {
                return Ok(derived_key);
            }
        }
    }

    // Add a new pod
    pub async fn add_pod(&mut self, data: &str, key_store: &mut KeyStore) -> Result<(String, String), String> {
        // Derive a new key for the pod scratchpad
        let scratchpad_key: SecretKey = self.create_scratchpad_key(key_store).await.map_err(|e| {
            println!("Error creating scratchpad key: {e}");
            format!("Error creating scratchpad key: {e}")
        })?;
        
        // Create new publicly readable scratchpad
        let scratchpad_address: ScratchpadAddress = ScratchpadAddress::new(scratchpad_key.clone().public_key());
        let scratchpad: Scratchpad = Scratchpad::new_with_signature(
            scratchpad_key.clone().public_key(),
            0,
            Bytes::from(data.to_owned()),
            0,
            scratchpad_key.sign(Scratchpad::bytes_for_signature(
                scratchpad_address.clone(),
                0,
                &Bytes::from(data.to_owned()),
                0,
            )),
        );

        // Derive a new key for the pod pointer
        let pointer_key: SecretKey = self.create_pointer_key(key_store).await.map_err(|e| {
            println!("Error creating pointer key: {e}");
            format!("Error creating pointer key: {e}")
        })?;

        // Create new pointer that points to the scratchpad
        let pointer = Pointer::new(
            &pointer_key,
            0,
            PointerTarget::ScratchpadAddress(scratchpad_address),
        );

        //FIXME: batch the pod scratchpad and pointer put operations
        // Put the scratchpad on the network
        let payment_option = PaymentOption::from(&self.wallet);
        let (scratchpad_cost, scratchpad_address) = self.client.scratchpad_put(scratchpad, payment_option.clone()).await.map_err(|e| {
            println!("Error putting scratchpad on network: {e}");
            format!("Error putting scratchpad on network: {e}")
        })?;
        let (pointer_cost, pointer_address) = self.client.pointer_put(pointer, payment_option).await.map_err(|e| {
            println!("Error putting pointer on network: {e}");
            format!("Error putting pointer on network: {e}")
        })?;
        println!("Scratchpad address: {scratchpad_address:?}");
        println!("Scratchpad cost: {scratchpad_cost:?}");
        println!("Pointer address: {pointer_address:?}");
        println!("Pointer cost: {pointer_cost:?}");

        Ok((pointer_address.to_string(), scratchpad_address.to_string()))
    }

    // Get pod data
    pub async fn get_pod_data(&mut self, address: String, key_store: &mut KeyStore) -> Result<String, String> {
        // get pointer
        let pointer_address = PointerAddress::from_hex(address.as_str()).unwrap();
        let pointer = self.client.pointer_get(&pointer_address).await.unwrap();
        let pointer_target = pointer.target();
        let pointer_target_string = pointer_target.to_hex();
        println!("Pointer target address: {}", pointer_target_string);

        // get scratchpad
        let scratchpad_address = ScratchpadAddress::from_hex(pointer_target_string.as_str()).unwrap();        // Lookup the key for the pod pointer from the key store
        let scratchpad = self.client.scratchpad_get(&scratchpad_address).await.unwrap();
        let scratchpad_data: String = String::from_utf8(scratchpad.encrypted_data().to_vec()).unwrap();
        println!("Scratchpad data: {}", scratchpad_data);
        Ok(scratchpad_data)
    }

    // Update pod
    pub async fn update_pod_data(&mut self, address: String, data: &str, key_store: &mut KeyStore) -> Result<(), String> {
        // get pointer
        let pointer_address = PointerAddress::from_hex(address.as_str()).unwrap();
        let pointer = self.client.pointer_get(&pointer_address).await.unwrap();
        let pointer_target = pointer.target();
        let pointer_target_string = pointer_target.to_hex();
        println!("Pointer target address: {}", pointer_target_string);

        // get scratchpad
        let scratchpad_address = ScratchpadAddress::from_hex(pointer_target_string.as_str()).unwrap();        // Lookup the key for the pod pointer from the key store
        let scratchpad = self.client.scratchpad_get(&scratchpad_address).await.unwrap();

        // Update the scratchpad contents and its counter
        let scratchpad_key = SecretKey::from_hex(key_store.get_pod_key(scratchpad_address.to_hex()).as_str()).unwrap();
        let scratchpad = Scratchpad::new_with_signature(
            scratchpad_key.clone().public_key(),
            0,
            Bytes::from(data.to_owned()),
            scratchpad.counter() + 1,
            scratchpad_key.sign(Scratchpad::bytes_for_signature(
                scratchpad_address.clone(),
                0,
                &Bytes::from(data.to_owned()),
                scratchpad.counter() + 1,
            )),
        );

        // Put the new scratchpad on the network
        let payment_option = PaymentOption::from(&self.wallet);
        let (scratchpad_cost, scratchpad_address) = self.client.scratchpad_put(scratchpad, payment_option.clone()).await.map_err(|e| {
            println!("Error putting scratchpad on network: {e}");
            format!("Error putting scratchpad on network: {e}")
        })?;
        println!("Scratchpad update cost: {scratchpad_cost:?}");

        // Update the pointer counter
        let pointer_key = SecretKey::from_hex(key_store.get_pod_key(pointer_address.to_hex()).as_str()).unwrap();
        self.client.pointer_update(&pointer_key, pointer_target.to_owned()).await.map_err(|e| {
            println!("Error updating pointer on network: {e}");
            format!("Error updating pointer on network: {e}")
        })?;

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

