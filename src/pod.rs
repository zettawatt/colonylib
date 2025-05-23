use autonomi::{Bytes, Client, SecretKey, Wallet, AddressParseError};
use autonomi::client::pointer::{Pointer, PointerTarget, PointerError, PointerAddress};
use autonomi::client::ConnectError;
use autonomi::client::scratchpad::{Scratchpad, ScratchpadError, ScratchpadAddress};
use autonomi::client::payment::PaymentOption;
use autonomi;
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

//#[derive(Clone)]
pub struct PodManager<'a> {
    pub client: Client,
    pub wallet: &'a Wallet,
    pub data_store: &'a mut DataStore,
    pub key_store: &'a mut KeyStore,
}

impl<'a> fmt::Debug for PodManager<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Network")
            .field("client", &"Client(Debug not implemented)")
            .field("wallet", &self.wallet.address().to_string())
            .field("data_store", &"DataStore(Debug not implemented)")
            .field("key_store", &"KeyStore(Debug not implemented)")
            .finish()
    }
}

impl<'a> PodManager<'a> {

    /// Initialize the client and wallet
    pub async fn new(client: Client,
                     wallet: &'a Wallet,
                     data_store: &'a mut DataStore,
                     key_store: &'a mut KeyStore) -> Result<Self, Error> {

        Ok(Self { client, wallet, data_store, key_store })
    }


    // Create a new pointer key, make sure it is empty, and add it to the key store
    #[instrument]
    async fn create_pointer_key(&mut self) -> Result<SecretKey, Error> {
        loop {
            // Derive a new key
            let key_string = self.key_store.add_derived_key()?;
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
    async fn create_scratchpad_key(&mut self) -> Result<SecretKey, Error> {
        loop {
            // Derive a new key
            let key_string = self.key_store.add_derived_key()?;
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

    ///////////////////////////////////////////
    // Local data operations
    ///////////////////////////////////////////

    // Add a new pod to the local data store
    #[instrument]
    pub fn add(&mut self, pod_id: &str) -> Result<(), Error> {
        Ok(())
    }

    // Update a pod in the local data store
    #[instrument]
    pub fn update(&mut self, pod_id: &str) -> Result<(), Error> {
        Ok(())
    }

    // Get a pod from the local data store
    #[instrument]
    pub fn get(&mut self, pod_id: &str) -> Result<String, Error> {
        let pod_data = self.data_store.read(pod_id)?;
        Ok(pod_data)
    }

    ///////////////////////////////////////////
    // Autonomi network operations
    ///////////////////////////////////////////
    // Create a new pod on the network
    #[instrument]
    pub async fn create(&mut self, data: &str) -> Result<(String, String), Error> {
        // Derive a new key for the pod scratchpad
        let scratchpad_key: SecretKey = self.create_scratchpad_key().await?;
        
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
        let pointer_key: SecretKey = self.create_pointer_key().await?;

        // Create new pointer that points to the scratchpad
        let pointer = Pointer::new(
            &pointer_key,
            0,
            PointerTarget::ScratchpadAddress(scratchpad_address),
        );

        //FIXME: batch the pod scratchpad and pointer put operations
        // Put the scratchpad on the network
        let payment_option = PaymentOption::from(self.wallet);
        let (scratchpad_cost, scratchpad_address) = self.client.scratchpad_put(scratchpad, payment_option.clone()).await?;
        let (pointer_cost, pointer_address) = self.client.pointer_put(pointer, payment_option).await?;
        debug!("Scratchpad address: {scratchpad_address:?}");
        debug!("Scratchpad cost: {scratchpad_cost:?}");
        debug!("Pointer address: {pointer_address:?}");
        debug!("Pointer cost: {pointer_cost:?}");

        Ok((pointer_address.to_string(), scratchpad_address.to_string()))
    }

    // Get pod data from the network
    #[instrument]
    pub async fn download(&mut self, address: String) -> Result<String, Error> {
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

    // Update pod data on the network
    #[instrument]
    pub async fn upload(&mut self, address: String, data: &str) -> Result<(), Error> {
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
        let scratchpad_key = SecretKey::from_hex(self.key_store.get_pod_key(scratchpad_address.to_hex())?.as_str())?;
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
        let payment_option = PaymentOption::from(self.wallet);
        let (scratchpad_cost, _scratchpad_address) = self.client.scratchpad_put(scratchpad, payment_option.clone()).await?;
        println!("Scratchpad update cost: {scratchpad_cost:?}");

        // Update the pointer counter
        let pointer_key = SecretKey::from_hex(self.key_store.get_pod_key(pointer_address.to_hex())?.as_str())?;
        self.client.pointer_update(&pointer_key, pointer_target.to_owned()).await?;

        Ok(()) //FIXME: need a return value for a success??
    }

    // Refresh pod cache from the network
    #[instrument]
    pub async fn refresh(&mut self) -> Result<(), String> {
        // Get the list of pods from the key store

        // Go through each pointer and check if there is an update vs the cache

        // If the pointer is newer, download and update the associated scratchpad and set the depth attribute

        // Recurse through each of the pods listed in the scratchpad and perform the same operation, increasing the depth attribute

        Ok(())
    }

}

