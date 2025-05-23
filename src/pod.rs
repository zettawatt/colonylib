use autonomi::{Bytes, Client, SecretKey, Wallet, AddressParseError};
use autonomi::client::pointer::{Pointer, PointerTarget, PointerError, PointerAddress};
use autonomi::client::ConnectError;
use autonomi::client::scratchpad::{Scratchpad, ScratchpadError, ScratchpadAddress};
use autonomi::client::payment::PaymentOption;
use autonomi;
use ruint::Uint;
use thiserror;
use tracing::{debug, error, info, warn, instrument};
use std::fmt;
use serde;
use blsttc::Error as BlsttcError;
use alloc::string::FromUtf8Error;

use crate::KeyStore;
use crate::key::Error as KeyStoreError;
use crate::DataStore;
use crate::data::Error as DataStoreError;


// Error handling
#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error(transparent)]
  Connect(#[from] ConnectError),
  #[error(transparent)]
  Pointer(#[from] PointerError),
  #[error(transparent)]
  Scratchpad(#[from] ScratchpadError),
  #[error(transparent)]
  Blsttc(#[from] BlsttcError),
  #[error(transparent)]
  Address(#[from] AddressParseError),
  #[error(transparent)]
  FromUtf8(#[from] FromUtf8Error),
  #[error(transparent)]
  KeyStore(#[from] KeyStoreError),
  #[error(transparent)]
  DataStore(#[from] DataStoreError),
}

#[derive(serde::Serialize)]
#[serde(tag = "kind", content = "message")]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    Connect(String),
    Pointer(String),
    Scratchpad(String),
    Blsttc(String),
    Address(String),
    FromUtf8(String),
    KeyStore(String),
    DataStore(String),
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
      S: serde::ser::Serializer,
    {
      let error_message = self.to_string();
      let error_kind = match self {
        Self::Connect(_) => ErrorKind::Connect(error_message),
        Self::Pointer(_) => ErrorKind::Pointer(error_message),
        Self::Scratchpad(_) => ErrorKind::Scratchpad(error_message),
        Self::Blsttc(_) => ErrorKind::Blsttc(error_message),
        Self::Address(_) => ErrorKind::Address(error_message),
        Self::FromUtf8(_) => ErrorKind::FromUtf8(error_message),
        Self::KeyStore(_) => ErrorKind::KeyStore(error_message),
        Self::DataStore(_) => ErrorKind::DataStore(error_message),
      };
      error_kind.serialize(serializer)
    }
  }

#[derive(Clone)]
pub struct PodManager {
    pub client: Client,
    pub wallet: Wallet,
}

impl fmt::Debug for PodManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Network")
            .field("client", &"Client(Debug not implemented)")
            .field("wallet", &self.wallet.address().to_string())
            .finish()
    }
}

impl PodManager {

    /// Initialize the client and wallet
    #[instrument]
    pub async fn new(wallet_key: String, environment: String) -> Result<Self, Error> {
        info!("Initializing client with environment: {environment:?}");

        let client = init_client(environment).await?;
        info!("Client initialized");

        let evm_network = client.evm_network();
        debug!("EVM network: {evm_network:?}");

        //FIXME: need to grap the wallet error and remove this unwrap()
        let wallet = Wallet::new_from_private_key(evm_network.clone(), wallet_key.as_str()).unwrap();
        debug!("Wallet loaded");

        Ok(Self { wallet, client })
    }

    // Get balance of gas tokens in wallet
    #[instrument]
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
    #[instrument]
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
    #[instrument]
    async fn create_pointer_key(&mut self, key_store: &mut KeyStore) -> Result<SecretKey, Error> {
        loop {
            // Derive a new key
            let key_string = key_store.add_derived_key()?;
            let derived_key: SecretKey = SecretKey::from_hex(key_string.as_str())?;
            // Check if the key is empty
            let address = PointerAddress::new(derived_key.clone().public_key());
            let already_exists = self.client.pointer_check_existance(&address).await?;
            if already_exists {
                warn!("Pointer key already exists, generating a new one");
                continue;
            } else {
                return Ok(derived_key);
            }
        }
    }

    // Create a new pointer key, make sure it is empty, and add it to the key store
    #[instrument]
    async fn create_scratchpad_key(&mut self, key_store: &mut KeyStore) -> Result<SecretKey, Error> {
        loop {
            // Derive a new key
            let key_string = key_store.add_derived_key()?;
            let derived_key: SecretKey = SecretKey::from_hex(key_string.as_str())?;
            // Check if the key is empty
            let address = ScratchpadAddress::new(derived_key.clone().public_key());
            let already_exists = self.client.scratchpad_check_existance(&address).await?;
            if already_exists {
                warn!("Scratchpad key already exists, generating a new one");
                continue;
            } else {
                return Ok(derived_key);
            }
        }
    }

    // Add a new pod
    #[instrument]
    pub async fn add_pod(&mut self, data: &str, key_store: &mut KeyStore) -> Result<(String, String), Error> {
        // Derive a new key for the pod scratchpad
        let scratchpad_key: SecretKey = self.create_scratchpad_key(key_store).await?;
        
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
        let pointer_key: SecretKey = self.create_pointer_key(key_store).await?;

        // Create new pointer that points to the scratchpad
        let pointer = Pointer::new(
            &pointer_key,
            0,
            PointerTarget::ScratchpadAddress(scratchpad_address),
        );

        //FIXME: batch the pod scratchpad and pointer put operations
        // Put the scratchpad on the network
        let payment_option = PaymentOption::from(&self.wallet);
        let (scratchpad_cost, scratchpad_address) = self.client.scratchpad_put(scratchpad, payment_option.clone()).await?;
        let (pointer_cost, pointer_address) = self.client.pointer_put(pointer, payment_option).await?;
        debug!("Scratchpad address: {scratchpad_address:?}");
        debug!("Scratchpad cost: {scratchpad_cost:?}");
        debug!("Pointer address: {pointer_address:?}");
        debug!("Pointer cost: {pointer_cost:?}");

        Ok((pointer_address.to_string(), scratchpad_address.to_string()))
    }

    // Get pod data
    #[instrument]
    pub async fn download_pod(&mut self, address: String, key_store: &mut KeyStore) -> Result<String, Error> {
        // get pointer
        let pointer_address = PointerAddress::from_hex(address.as_str())?;
        let pointer = self.client.pointer_get(&pointer_address).await?;
        let pointer_target = pointer.target();
        let pointer_target_string = pointer_target.to_hex();
        debug!("Pointer target address: {}", pointer_target_string);

        // get scratchpad
        let scratchpad_address = ScratchpadAddress::from_hex(pointer_target_string.as_str())?;        // Lookup the key for the pod pointer from the key store
        let scratchpad = self.client.scratchpad_get(&scratchpad_address).await?;
        let scratchpad_data: String = String::from_utf8(scratchpad.encrypted_data().to_vec())?;
        debug!("Scratchpad data: {}", scratchpad_data);
        Ok(scratchpad_data)
    }

    // Update pod
    #[instrument]
    pub async fn upload_pod(&mut self, address: String, data: &str, key_store: &mut KeyStore) -> Result<(), Error> {
        // get pointer
        let pointer_address = PointerAddress::from_hex(address.as_str())?;
        let pointer = self.client.pointer_get(&pointer_address).await?;
        let pointer_target = pointer.target();
        let pointer_target_string = pointer_target.to_hex();
        println!("Pointer target address: {}", pointer_target_string);

        // get scratchpad
        let scratchpad_address = ScratchpadAddress::from_hex(pointer_target_string.as_str())?;        // Lookup the key for the pod pointer from the key store
        let scratchpad = self.client.scratchpad_get(&scratchpad_address).await?;

        // Update the scratchpad contents and its counter
        let scratchpad_key = SecretKey::from_hex(key_store.get_pod_key(scratchpad_address.to_hex())?.as_str())?;
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
        let (scratchpad_cost, _scratchpad_address) = self.client.scratchpad_put(scratchpad, payment_option.clone()).await?;
        println!("Scratchpad update cost: {scratchpad_cost:?}");

        // Update the pointer counter
        let pointer_key = SecretKey::from_hex(key_store.get_pod_key(pointer_address.to_hex())?.as_str())?;
        self.client.pointer_update(&pointer_key, pointer_target.to_owned()).await?;

        Ok(()) //FIXME: need a return value for a success??
    }

    // Refresh pod cache
    #[instrument]
    pub async fn refresh_pod_cache(self, key_store: KeyStore) -> Result<(), String> {
        // Get the list of pods from the key store

        // Go through each pointer and check if there is an update vs the cache

        // If the pointer is newer, download and update the associated scratchpad and set the depth attribute

        // Recurse through each of the pods listed in the scratchpad and perform the same operation, increasing the depth attribute

        Ok(()) //FIXME: need a return value for a success??
    }

}

async fn init_client(environment: String) -> Result<Client, Error> {
    match environment.as_str() {
        "local" => Client::init_local().await.map_err(Error::Connect),
        "alpha" => Client::init_alpha().await.map_err(Error::Connect),
        _ => Client::init().await.map_err(Error::Connect), // "autonomi"
    }
}

