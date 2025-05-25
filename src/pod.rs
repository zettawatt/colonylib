use autonomi::{AddressParseError, Bytes, Chunk, Client, SecretKey, Wallet};
use autonomi::client::pointer::{Pointer, PointerTarget, PointerError, PointerAddress};
use autonomi::client::ConnectError;
use autonomi::client::scratchpad::{Scratchpad, ScratchpadError, ScratchpadAddress};
use autonomi::client::payment::PaymentOption;
use autonomi;
use std::fs::File;
use std::io::{BufReader, BufRead};
use thiserror;
use tracing::{debug, error, info, warn, instrument};
use std::fmt;
use serde;
use blsttc::Error as BlsttcError;
use alloc::string::FromUtf8Error;
use std::io::Error as IoError;
use autonomi::client::analyze::{AnalysisError, Analysis};

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
  #[error(transparent)]
  Io(#[from] IoError),
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
    Io(String),
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
        Self::Io(_) => ErrorKind::Io(error_message),
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
    async fn create_key(&mut self) -> Result<SecretKey, Error> {
        loop {
            // Derive a new key
            let key_string = self.key_store.add_derived_key()?;
            let derived_key: SecretKey = SecretKey::from_hex(key_string.as_str())?;
            
            // Check if the key is empty
            match self.client.analyze_address(&derived_key.public_key().to_hex().as_str(), false).await {
                Ok(_) => continue, // If analysis succeeds, there is data at the address already, continue the loop
                Err(AnalysisError::FailedGet) => {
                    return Ok(derived_key); // Exit the loop and return the key
                }
                Err(AnalysisError::UnrecognizedInput) => {
                    warn!("Unrecognized input, generating a new key");
                    continue; // Continue the loop for this error
                }
                Err(AnalysisError::GetError(get_error)) => {
                    warn!("Get error: {:?}", get_error);
                    continue; // Continue the loop for this error
                }
            }
        }
    }

    ///////////////////////////////////////////
    // Local data operations
    ///////////////////////////////////////////

    // Add a new pod to the local data store
    #[instrument]
    pub async fn add(&mut self) -> Result<(String,String), Error> {
        let scratchpad_address = self.add_scratchpad().await?;
        let pointer_address = self.add_pointer().await?;

        // Add the scratchpad address to the pointer file
        let _ = self.data_store.update_pointer_target(pointer_address.clone().to_hex().as_str(), scratchpad_address.clone().to_hex().as_str())?;

        Ok((pointer_address.to_string(), scratchpad_address.to_string()))
    }

    async fn add_scratchpad(&mut self) -> Result<ScratchpadAddress, Error> {
        // Derive a new key for the pod scratchpad
        let scratchpad_key: SecretKey = self.create_key().await?;
        let scratchpad_address: ScratchpadAddress = ScratchpadAddress::new(scratchpad_key.clone().public_key());

        // Create a new file in the pod directory from the address
        let _ = self.data_store.create_scratchpad_file(scratchpad_address.clone().to_hex().as_str())?;
        self.data_store.append_update_list(scratchpad_address.clone().to_hex().as_str())?;

        Ok(scratchpad_address)
    }

    async fn add_pointer(&mut self) -> Result<PointerAddress, Error> {
        // Derive a new key for the pod scratchpad
        let pointer_key: SecretKey = self.create_key().await?;
        let pointer_address = PointerAddress::new(pointer_key.clone().public_key());

        // Create a new file in the pod directory from the address
        let _ = self.data_store.create_pointer_file(pointer_address.clone().to_hex().as_str())?;
        self.data_store.append_update_list(pointer_address.clone().to_hex().as_str())?;

        Ok(pointer_address)
    }

    // Update a pod in the local data store
    #[instrument]
    pub fn update(&mut self, address: &str, data: &str) -> Result<(), Error> {
        // Get the scratchpad address from the pointer
        let scratchpad_address = self.data_store.get_pointer_target(address)?;
        // Update the scratchpad data
        let _ = self.data_store.update_scratchpad_data(scratchpad_address.as_str(), data)?;

        // Add the addres and scratchpad address to the update list
        let _ = self.data_store.append_update_list(address)?;
        let _ = self.data_store.append_update_list(scratchpad_address.as_str())?;

        Ok(())
    }

    // Get a pod from the local data store
    #[instrument]
    pub fn get(&mut self, address: &str) -> Result<String, Error> {
        let scratchpad_address = self.data_store.get_pointer_target(address)?;
        let pod_data = self.data_store.get_scratchpad_data(scratchpad_address.as_str())?;
        Ok(pod_data)
    }

    ///////////////////////////////////////////
    // Autonomi network operations
    ///////////////////////////////////////////
    
    pub async fn upload_all(&mut self) -> Result<(), Error> {
        // open update list and walk through each line
        let file_path = self.data_store.get_update_list_path();
        let file = File::open(file_path.clone())?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            let address = line.trim();
            let mut create_mode = false;
            debug!("Uploading pod: {}", address);
            
            // get the type stored on the network
            let pod_type = self.client.analyze_address(address, false).await.unwrap_or_else(|e| -> Analysis {
                match e {
                    AnalysisError::FailedGet => {
                        info!("Address currently does not hold data: {}", address);
                        create_mode = true;
                        // check if address is a directory (pointer) or a file (scratchpad)
                        // and return a dummy analysis type for processing, else
                        // return a chunk to indicate an error
                        if self.data_store.address_is_pointer(address).unwrap_or(false) {
                            Analysis::Pointer(Pointer::new(
                                &SecretKey::from_hex(self.key_store.get_pod_key(address.to_string()).unwrap().as_str()).unwrap(),
                                0,
                                PointerTarget::ScratchpadAddress(ScratchpadAddress::new(SecretKey::from_hex(self.key_store.get_pod_key(address.to_string()).unwrap().as_str()).unwrap().public_key())),
                            ))
                        } else if self.data_store.address_is_scratchpad(address).unwrap_or(false) {
                            Analysis::Scratchpad(Scratchpad::new(
                                &SecretKey::from_hex(self.key_store.get_pod_key(address.to_string()).unwrap().as_str()).unwrap(),
                                0,
                                &Bytes::new(),
                                0))
                        } else {
                            error!("Address is neither a pointer nor a scratchpad: {}", address);
                            Analysis::Chunk(Chunk::new(Bytes::new()))
                        }
                    }
                    _ => {
                        error!("Address error: {}", e);
                        Analysis::Chunk(Chunk::new(Bytes::new()))
                    }
                }
            });
            debug!("Pod type: {:?}", pod_type);

            match pod_type {
                Analysis::Pointer(_) => {
                    let target = self.data_store.get_pointer_target(address)?;
                    if create_mode {
                        // Create new pointer
                        info!("Nothing stored at address, creating new pointer");
                        let _ = self.create_pointer(address, target.as_str()).await?;
                    } else {
                        // Update existing pointer
                        info!("Object stored at address is a pointer");
                        let _ = self.update_pointer(address, target.as_str()).await?;
                    }
                }
                Analysis::Scratchpad(_) => {
                    let data = self.data_store.get_scratchpad_data(address)?;
                    if create_mode {
                        // Create new scratchpad
                        info!("Nothing stored at address, creating new scratchpad");
                        let _ = self.create_scratchpad(address, data.as_str()).await?;
                    } else {
                        // Update existing scratchpad
                        info!("Object stored at address is a scratchpad");
                        let _ = self.update_scratchpad(address, data.as_str()).await?;
                    }
                }
                _ => {
                    error!("Pod type is unknown, skipping upload");
                    continue;
                }
            }
            
        }

        // Clear out the update list
        let _ = File::create(file_path)?;
        Ok(())
    }

    async fn create_pointer(&mut self, address: &str, target: &str) -> Result<String, Error> {
        let key_string = self.key_store.get_pod_key(address.to_string())?;
        let key: SecretKey = SecretKey::from_hex(key_string.as_str())?;

        // Create new pointer that points to the scratchpad
        let pointer = Pointer::new(
            &key,
            0,
            PointerTarget::ScratchpadAddress(ScratchpadAddress::from_hex(target)?),
        );

        // Put the pointer on the network
        let payment_option = PaymentOption::from(self.wallet);
        let (pointer_cost, _pointer_address) = self.client.pointer_put(pointer, payment_option).await?;
        debug!("Pointer upload cost: {pointer_cost:?}");

        Ok(pointer_cost.to_string())
    }

    async fn create_scratchpad(&mut self, address: &str, data: &str) -> Result<String, Error> {
        let key_string = self.key_store.get_pod_key(address.to_string())?;
        let key: SecretKey = SecretKey::from_hex(key_string.as_str())?;
        
        // Create new publicly readable scratchpad
        let scratchpad_address: ScratchpadAddress = ScratchpadAddress::new(key.clone().public_key());
        let scratchpad: Scratchpad = Scratchpad::new_with_signature(
            key.clone().public_key(),
            0,
            Bytes::from(data.to_owned()),
            0,
            key.sign(Scratchpad::bytes_for_signature(
                scratchpad_address,
                0,
                &Bytes::from(data.to_owned()),
                0,
            )),
        );

        // Put the scratchpad on the network
        let payment_option = PaymentOption::from(self.wallet);
        let (scratchpad_cost, _scratchpad_address) = self.client.scratchpad_put(scratchpad, payment_option.clone()).await?;
        debug!("Scratchpad cost: {scratchpad_cost:?}");

        Ok(scratchpad_cost.to_string())
    }

    async fn update_pointer(&mut self, address: &str, target: &str) -> Result<(), Error> {
        let key_string = self.key_store.get_pod_key(address.to_string())?;
        let key: SecretKey = SecretKey::from_hex(key_string.as_str())?;

        // get pointer to make sure it exists
        let pointer_address = PointerAddress::from_hex(address)?;
        let _pointer = self.client.pointer_get(&pointer_address).await?;

        // Create the target address
        let target_address = ScratchpadAddress::from_hex(target)?;
        let target = PointerTarget::ScratchpadAddress(target_address);

        // Update the pointer counter and target 
        self.client.pointer_update(&key, target).await?;
        Ok(())
    }

    async fn update_scratchpad(&mut self, address: &str, data: &str) -> Result<(), Error> {
        let key_string = self.key_store.get_pod_key(address.to_string())?;
        let key: SecretKey = SecretKey::from_hex(key_string.as_str())?;

        // get the scratchpad to make sure it exists and to get the current counter value
        let scratchpad_address = ScratchpadAddress::from_hex(address)?;        // Lookup the key for the pod pointer from the key store
        let scratchpad = self.client.scratchpad_get(&scratchpad_address).await?;

        // Update the scratchpad contents and its counter
        let scratchpad = Scratchpad::new_with_signature(
            key.clone().public_key(),
            0,
            Bytes::from(data.to_owned()),
            scratchpad.counter() + 1,
            key.sign(Scratchpad::bytes_for_signature(
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

        Ok(())
    }

    // Create a new pod on the network
    #[instrument]
    pub async fn create(&mut self, data: &str) -> Result<(String, String), Error> {
        // Derive a new key for the pod scratchpad
        let scratchpad_key: SecretKey = self.create_key().await?;
        
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
        let pointer_key: SecretKey = self.create_key().await?;

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

