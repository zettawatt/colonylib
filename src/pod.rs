use autonomi::{AddressParseError, Bytes, Chunk, Client, SecretKey, Wallet};
use autonomi::client::pointer::{Pointer, PointerTarget, PointerError, PointerAddress};
use autonomi::client::ConnectError;
use autonomi::client::scratchpad::{Scratchpad, ScratchpadError, ScratchpadAddress};
use autonomi::client::payment::PaymentOption;
use autonomi;
use std::fs::File;
use std::io::{BufReader, BufRead};
use thiserror;
use tracing::{debug, error, info, warn};
use std::fmt;
use serde;
use blsttc::Error as BlsttcError;
use alloc::string::FromUtf8Error;
use std::io::Error as IoError;
use autonomi::client::analyze::{AnalysisError, Analysis};
use serde_json::{Value, Error as SerdeError};

use crate::KeyStore;
use crate::key::Error as KeyStoreError;
use crate::DataStore;
use crate::data::Error as DataStoreError;
use crate::Graph;
use crate::graph::Error as GraphError;


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
  #[error(transparent)]
  Serde(#[from] SerdeError),
  #[error(transparent)]
  Graph(#[from] GraphError),
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
    Serde(String),
    Graph(String),
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
        Self::Serde(_) => ErrorKind::Serde(error_message),
        Self::Graph(_) => ErrorKind::Graph(error_message),
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
    pub graph: &'a mut Graph,
}

impl<'a> fmt::Debug for PodManager<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PodManager")
            .field("client", &"Client(Debug not implemented)")
            .field("wallet", &self.wallet.address().to_string())
            .field("data_store", &"DataStore(Debug not implemented)")
            .field("key_store", &"KeyStore(Debug not implemented)")
            .field("graph", &"Graph(Debug not implemented)")
            .finish()
    }
}

impl<'a> PodManager<'a> {

    /// Initialize the client and wallet
    pub async fn new(client: Client,
                     wallet: &'a Wallet,
                     data_store: &'a mut DataStore,
                     key_store: &'a mut KeyStore,
                     graph: &'a mut Graph) -> Result<Self, Error> {

        Ok(Self { client, wallet, data_store, key_store, graph })
    }

    // Create a new pointer key, make sure it is empty, and add it to the key store
    async fn create_pointer_key(&mut self) -> Result<SecretKey, Error> {
        loop {
            // Derive a new key
            info!("Deriving a new key");
            let key_string = self.key_store.add_pointer_key()?;
            info!("Newly derived key: {}", key_string);
            let derived_key: SecretKey = SecretKey::from_hex(key_string.trim())?;
            
            // Check if the key is empty
            match self.client.analyze_address(&derived_key.public_key().to_hex().as_str(), false).await {
                Ok(_) => continue, // If analysis succeeds, there is data at the address already, continue the loop
                Err(AnalysisError::FailedGet) => {
                    info!("Address is empty, using it for the pod");
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

    // Create a new scratchpad key, make sure it is empty, and add it to the key store
    async fn create_scratchpad_key(&mut self) -> Result<SecretKey, Error> {
        loop {
            // Derive a new key
            info!("Deriving a new key");
            let key_string = self.key_store.add_scratchpad_key()?;
            info!("Newly derived key: {}", key_string);
            let derived_key: SecretKey = SecretKey::from_hex(key_string.trim())?;
            
            // Check if the key is empty
            match self.client.analyze_address(&derived_key.public_key().to_hex().as_str(), false).await {
                Ok(_) => continue, // If analysis succeeds, there is data at the address already, continue the loop
                Err(AnalysisError::FailedGet) => {
                    info!("Address is empty, using it for the pod");
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
    // Graph operations
    ///////////////////////////////////////////

    // Search for content
    pub async fn search(&mut self, query: Value) -> Result<Value, Error> {
        info!("Performing search with query: {}", query);

        // Parse the query to determine search type and parameters
        let search_results = if let Some(query_obj) = query.as_object() {
            // Handle structured query
            if let Some(search_type) = query_obj.get("type").and_then(|v| v.as_str()) {
                match search_type {
                    "text" => {
                        // Text search across all literal values
                        if let Some(text) = query_obj.get("text").and_then(|v| v.as_str()) {
                            let limit = query_obj.get("limit").and_then(|v| v.as_u64());
                            self.graph.search_content(text, limit)?
                        } else {
                            return Ok(Value::Object(serde_json::Map::from_iter([
                                ("error".to_string(), Value::String("Missing 'text' parameter for text search".to_string()))
                            ])));
                        }
                    },
                    "by_type" => {
                        // Search by RDF type
                        if let Some(type_uri) = query_obj.get("type_uri").and_then(|v| v.as_str()) {
                            let limit = query_obj.get("limit").and_then(|v| v.as_u64());
                            self.graph.search_by_type(type_uri, limit)?
                        } else {
                            return Ok(Value::Object(serde_json::Map::from_iter([
                                ("error".to_string(), Value::String("Missing 'type_uri' parameter for type search".to_string()))
                            ])));
                        }
                    },
                    "by_predicate" => {
                        // Search by predicate
                        if let Some(predicate_uri) = query_obj.get("predicate_uri").and_then(|v| v.as_str()) {
                            let limit = query_obj.get("limit").and_then(|v| v.as_u64());
                            self.graph.search_by_predicate(predicate_uri, limit)?
                        } else {
                            return Ok(Value::Object(serde_json::Map::from_iter([
                                ("error".to_string(), Value::String("Missing 'predicate_uri' parameter for predicate search".to_string()))
                            ])));
                        }
                    },
                    "advanced" => {
                        // Advanced search with multiple criteria
                        self.graph.advanced_search(&query)?
                    },
                    _ => {
                        return Ok(Value::Object(serde_json::Map::from_iter([
                            ("error".to_string(), Value::String(format!("Unknown search type: {}", search_type)))
                        ])));
                    }
                }
            } else {
                // No explicit type, treat as advanced search
                self.graph.advanced_search(&query)?
            }
        } else if let Some(text) = query.as_str() {
            // Simple text search if query is just a string
            self.graph.search_content(text, Some(50))?
        } else {
            return Ok(Value::Object(serde_json::Map::from_iter([
                ("error".to_string(), Value::String("Invalid query format. Expected object or string.".to_string()))
            ])));
        };

        // Parse the SPARQL JSON results and return them
        let results: Value = serde_json::from_str(&search_results)?;

        // Enhance the results with additional metadata
        let enhanced_results = self.enhance_search_results(results)?;

        info!("Search completed successfully");
        Ok(enhanced_results)
    }

    // Helper method to enhance search results with additional metadata
    fn enhance_search_results(&self, results: Value) -> Result<Value, Error> {
        let mut enhanced = serde_json::Map::new();

        if let Some(results_obj) = results.as_object() {
            // Copy the original results
            enhanced.insert("sparql_results".to_string(), results.clone());

            // Add metadata
            enhanced.insert("search_timestamp".to_string(),
                Value::String(chrono::Utc::now().to_rfc3339()));

            // Count results
            if let Some(bindings) = results_obj.get("results")
                .and_then(|r| r.get("bindings"))
                .and_then(|b| b.as_array()) {
                enhanced.insert("result_count".to_string(),
                    Value::Number(serde_json::Number::from(bindings.len())));

                // Extract unique graphs (pods) from results
                let mut unique_graphs = std::collections::HashSet::new();
                for binding in bindings {
                    if let Some(graph_value) = binding.get("graph")
                        .and_then(|g| g.get("value"))
                        .and_then(|v| v.as_str()) {
                        unique_graphs.insert(graph_value.to_string());
                    }
                }

                let graphs_vec: Vec<Value> = unique_graphs.into_iter()
                    .map(|g| Value::String(g))
                    .collect();
                enhanced.insert("pods_found".to_string(), Value::Array(graphs_vec));
            } else {
                enhanced.insert("result_count".to_string(), Value::Number(serde_json::Number::from(0)));
                enhanced.insert("pods_found".to_string(), Value::Array(vec![]));
            }
        } else {
            // If results is not an object, just wrap it
            enhanced.insert("sparql_results".to_string(), results);
            enhanced.insert("result_count".to_string(), Value::Number(serde_json::Number::from(0)));
            enhanced.insert("pods_found".to_string(), Value::Array(vec![]));
        }

        Ok(Value::Object(enhanced))
    }
    // Add/modify/remove file metadata in a pod
    pub async fn put_subject_data(&mut self, pod_address: &str, subject_address: &str, subject_data: &str) -> Result<(), Error> {
        
        // Inject the JSON data into the graph using the pod address as the named graph
        // And return the resulting graph data as a TriG formatted byte vector
        let graph = self.graph.put_subject_data(pod_address, subject_address, subject_data)?;

        // Split the byte vector into 4MB chunks so that the data fits into scratchpads
        // TODO

        // Map the chunks to scratchpad addresses and update them with the new data
        // TODO, for now just write the whole graph to the scratchpad
        let pod_data: String = graph.into_iter().map(|b| b as char).collect();
        let scratchpad_address = self.data_store.get_pointer_target(pod_address)?;
        let _ = self.data_store.update_scratchpad_data(scratchpad_address.trim(), pod_data.as_str())?;

        // Add the pod pointer address and scratchpad addresses to the update list
        let _ = self.data_store.append_update_list(pod_address)?;

        let addresses = self.get_pod_scratchpads(pod_address)?;
        if let Some(addresses) = addresses {
            for addr in addresses {
                let _ = self.data_store.append_update_list(addr.trim())?;
            }
        }
        Ok(())
    }

    pub async fn get_subject_data(&mut self, subject_address: &str) -> Result<String, Error> {
        // Perform a SPARQL query with the Autonomi object address and return the metadata as JSON results
        let json_data = self.graph.get_subject_data(subject_address)?;
        
        Ok(json_data)
    }

    fn get_pod_scratchpads(&self, address: &str) -> Result<Option<Vec<String>>, Error> {
        // TODO: Placeholder function to get all pod scratchpad addresses from the pointer address
        // This will be implemented to read from the scratchpad data and extract addresses

        // For now, just return the pointer target as a single-item vector
        let target = self.data_store.get_pointer_target(address)?;
        Ok(Some(vec![target]))
    }

    ///////////////////////////////////////////
    // Local data operations
    ///////////////////////////////////////////
    

    // Add a new pod to the local data store
    pub async fn add_pod(&mut self) -> Result<(String,String), Error> {
        let scratchpad_address = self.add_scratchpad().await?;
        let scratchpad_address = scratchpad_address.to_hex();
        let scratchpad_address = scratchpad_address.as_str();
        let pointer_address = self.add_pointer().await?;
        let pointer_address = pointer_address.to_hex();
        let pointer_address = pointer_address.as_str();

        // Add the scratchpad address to the pointer file
        let _ = self.data_store.update_pointer_target(pointer_address, scratchpad_address)?;

        // Add initial data to the scratchpad
        let pod_data = self.graph.add_pod_entry(pointer_address, scratchpad_address)?;
        let _ = self.data_store.update_scratchpad_data(scratchpad_address, pod_data.as_str())?;
        Ok((pointer_address.to_string(), scratchpad_address.to_string()))
    }

    async fn add_scratchpad(&mut self) -> Result<ScratchpadAddress, Error> {
        // Derive a new key for the pod scratchpad
        let scratchpad_key: SecretKey = self.create_scratchpad_key().await?;
        let scratchpad_address: ScratchpadAddress = ScratchpadAddress::new(scratchpad_key.clone().public_key());

        // Create a new file in the pod directory from the address
        let _ = self.data_store.create_scratchpad_file(scratchpad_address.clone().to_hex().as_str())?;
        self.data_store.append_update_list(scratchpad_address.clone().to_hex().as_str())?;

        Ok(scratchpad_address)
    }

    async fn add_pointer(&mut self) -> Result<PointerAddress, Error> {
        // Derive a new key for the pod scratchpad
        let pointer_key: SecretKey = self.create_pointer_key().await?;
        let pointer_address = PointerAddress::new(pointer_key.clone().public_key());

        // Create a new file in the pod directory from the address
        let _ = self.data_store.create_pointer_file(pointer_address.clone().to_hex().as_str())?;
        self.data_store.append_update_list(pointer_address.clone().to_hex().as_str())?;

        Ok(pointer_address)
    }

    // Update a pod in the local data store
    // FIXME: will remove or make private once graph operations are implemented
    pub fn update_pod(&mut self, address: &str, data: &str) -> Result<(), Error> {
        // Get the scratchpad address from the pointer
        let scratchpad_address = self.data_store.get_pointer_target(address)?;
        // Update the scratchpad data
        let _ = self.data_store.update_scratchpad_data(scratchpad_address.trim(), data)?;

        // Add the addres and scratchpad address to the update list
        let _ = self.data_store.append_update_list(address)?;
        let _ = self.data_store.append_update_list(scratchpad_address.trim())?;

        Ok(())
    }

    // Get a pod from the local data store
    // FIXME: will remove or make private once graph operations are implemented
    pub fn get_pod(&mut self, address: &str) -> Result<String, Error> {
        let scratchpad_address = self.data_store.get_pointer_target(address)?;
        let pod_data = self.data_store.get_scratchpad_data(scratchpad_address.trim())?;
        Ok(pod_data)
    }

    ///////////////////////////////////////////
    // Autonomi network operations
    ///////////////////////////////////////////
     
    async fn get_address_type(&mut self, address: &str) -> Result<(Analysis, bool), Error> {
        // get the type stored on the network
        let mut create_mode = false;
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
                            &SecretKey::from_hex(self.key_store.get_pointer_key(address.to_string()).unwrap().trim()).unwrap(),
                            0,
                            PointerTarget::ScratchpadAddress(ScratchpadAddress::new(SecretKey::from_hex(self.key_store.get_pointer_key(address.to_string()).unwrap().trim()).unwrap().public_key())),
                        ))
                    } else if self.data_store.address_is_scratchpad(address).unwrap_or(false) {
                        Analysis::Scratchpad(Scratchpad::new(
                            &SecretKey::from_hex(self.key_store.get_scratchpad_key(address.to_string()).unwrap().trim()).unwrap(),
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
        Ok((pod_type, create_mode))
    }
    
    pub async fn upload_all(&mut self) -> Result<(), Error> {
        // open update list and walk through each line
        let file_path = self.data_store.get_update_list_path();
        let file = File::open(file_path.clone())?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            let address = line.trim();
            debug!("Uploading pod: {}", address);
            
            // get the type stored on the network
            let (address_type, create_mode) = self.get_address_type(address).await?;
            debug!("Pod type: {:?}", address_type);

            match address_type {
                Analysis::Pointer(_) => {
                    let target = self.data_store.get_pointer_target(address)?;
                    if create_mode {
                        // Create new pointer
                        info!("Nothing stored at address, creating new pointer");
                        let _ = self.create_pointer(address, target.trim()).await?;
                    } else {
                        // Update existing pointer
                        info!("Object stored at address is a pointer");
                        let _ = self.update_pointer(address, target.trim()).await?;
                    }
                }
                Analysis::Scratchpad(_) => {
                    let data = self.data_store.get_scratchpad_data(address)?;
                    if create_mode {
                        // Create new scratchpad
                        info!("Nothing stored at address, creating new scratchpad");
                        let _ = self.create_scratchpad(address, data.trim()).await?;
                    } else {
                        // Update existing scratchpad
                        info!("Object stored at address is a scratchpad");
                        let _ = self.update_scratchpad(address, data.trim()).await?;
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
        let key_string = self.key_store.get_pointer_key(address.to_string())?;
        let key: SecretKey = SecretKey::from_hex(key_string.trim())?;

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
        let key_string = self.key_store.get_scratchpad_key(address.to_string())?;
        let key: SecretKey = SecretKey::from_hex(key_string.trim())?;
        
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

        info!("Scratchpad payload size: {}", scratchpad.payload_size());
        info!("Scratchpad total size: {}", scratchpad.payload_size());

        // Put the scratchpad on the network
        let payment_option = PaymentOption::from(self.wallet);
        let (scratchpad_cost, _scratchpad_address) = self.client.scratchpad_put(scratchpad, payment_option.clone()).await?;
        debug!("Scratchpad cost: {scratchpad_cost:?}");

        Ok(scratchpad_cost.to_string())
    }

    async fn update_pointer(&mut self, address: &str, target: &str) -> Result<(), Error> {
        let key_string = self.key_store.get_pointer_key(address.to_string())?;
        let key: SecretKey = SecretKey::from_hex(key_string.trim())?;

        // get pointer to make sure it exists
        let pointer_address = PointerAddress::from_hex(address)?;
        let pointer = self.client.pointer_get(&pointer_address).await?;

        // Create the target address
        let target_address = ScratchpadAddress::from_hex(target)?;
        let target = PointerTarget::ScratchpadAddress(target_address);

        // Update the pointer counter and target 
        self.client.pointer_update(&key, target).await?;

        // Update the local pointer file counter
        let pointer_count = pointer.counter() + 1;
        self.data_store.update_pointer_count(address, pointer_count.into())?;
        Ok(())
    }

    async fn update_scratchpad(&mut self, address: &str, data: &str) -> Result<(), Error> {
        let key_string = self.key_store.get_scratchpad_key(address.to_string())?;
        let key: SecretKey = SecretKey::from_hex(key_string.trim())?;

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

    pub async fn refresh_cache(&mut self) -> Result<(), Error> {
        // Loop through the next 3 derived keys and check if they contain data on the network
        // This is to ensure that we have all of the relevant keys in our key store
        let mut count: u64 = 0;
        let base_count = count.clone();
        loop {
            let address = self.key_store.get_address_at_index(self.key_store.get_num_keys() as u64 + count)?;
            info!("Checking address: {}", address);
            let (address_type, create_mode) = self.get_address_type(address.as_str()).await?;
            if create_mode {
                info!("Address is empty, increment count!");
                count += 1;
            } else {
                info!("Address is not empty, processing type: {:?}", address_type);
                match address_type {
                    Analysis::Pointer(_) => {
                        info!("Address is a pointer, adding key");
                        self.key_store.add_pointer_key()?;
                    }
                    Analysis::Scratchpad(_) => {
                        info!("Address is a scratchpad, adding key");
                        self.key_store.add_scratchpad_key()?;
                    }
                    _ => {
                        info!("Address is neither a pointer nor a scratchpad, marking key as bad");
                        self.key_store.add_bad_key()?;
                    }
                }
                count = base_count;
            }
            if count > 2 {
                info!("No new addresses found, done with refresh!");
                break;
            }
        }

        // Get the list of local pointers from the key store
        for (address, _key) in self.key_store.get_pointers() {
            let address = address.trim();
            info!("Checking pointer: {}", address);
            let pointer_address = PointerAddress::from_hex(address)?;
            let pointer = self.client.pointer_get(&pointer_address).await?;
            info!("Pointer found: {:?}", pointer);

            // Check if the pointer file exists in the local data store
            let pointer_exists = self.data_store.address_is_pointer(address)?;
            if !pointer_exists {
                info!("Pointer file does not exist, creating it");
                self.data_store.create_pointer_file(address)?;
                self.data_store.update_pointer_target(address, pointer.target().to_hex().as_str())?;
                self.data_store.update_pointer_count(address, pointer.counter().into())?;

                // Check if the scratchpad file exists
                let target = pointer.target();
                let target = match target {
                    PointerTarget::ScratchpadAddress(scratchpad_address) => scratchpad_address,
                    _ => {
                        error!("Pointer target is not a scratchpad address, skipping");
                        continue;
                    }
                };
                if !self.data_store.address_is_scratchpad(target.to_hex().as_str())? {
                    info!("Scratchpad file does not exist, creating it");
                    self.data_store.create_scratchpad_file(target.to_hex().as_str())?;
                }
                // Download the scratchpad data
                let scratchpad = self.client.scratchpad_get(target).await?;
                let data = scratchpad.encrypted_data();
                let data = String::from_utf8(data.to_vec())?;
                self.data_store.update_scratchpad_data(target.to_hex().as_str(), data.trim())?;
                info!("Pointer and scratchpad files created successfully");
                continue; // Skip to the next pointer if it was just created
            }
            // Check if the pointer is newer than the local cache
            let local_pointer_count = self.data_store.get_pointer_count(address)?;
            if pointer.counter() as u64 > local_pointer_count {
                info!("Pointer is newer, updating scratchpad");
                let target = pointer.target();
                // get the scratchpad address from the pointer target
                let target = match target {
                    PointerTarget::ScratchpadAddress(scratchpad_address) => scratchpad_address,
                    _ => {
                        error!("Pointer target is not a scratchpad address, skipping");
                        continue;
                    }
                };
                let scratchpad = self.client.scratchpad_get(target).await?;
                let data = scratchpad.encrypted_data();
                let data = String::from_utf8(data.to_vec())?;
                self.data_store.update_scratchpad_data(target.to_hex().as_str(), data.trim())?;
                self.data_store.update_pointer_target(address, target.to_hex().as_str())?;
                self.data_store.update_pointer_count(address, pointer.counter().into())?;
                // FIXME: update graph database
                // clear existing pod graph if it exists using the store.clear function
                // read in all of the local scratchpads into the RDF graph database
                // Get any additional scratchpads that are part of this pod
                // Remove the existing scratchpad graph from the existing database
                // Add the newly downloaded scratchpad to the graph database
                // Set the depth attribute to 0
            } else {
                info!("Pointer is up to date");
            }
        }
        Ok(())
    }
 
    // Refresh pod cache from the network
    pub async fn refresh_ref(&mut self, depth: u64) -> Result<(), Error> {
        let _ = self.refresh_cache().await?;

        // Process pods iteratively up to the specified depth to avoid async recursion
        for current_depth in 0..=depth {
            info!("Processing pod references at depth {}", current_depth);

            // Get all local pods at the current depth
            let pod_addresses = self.get_pods_at_depth(current_depth)?;
            let mut referenced_pods = Vec::new();

            // Walk through each pod graph and check if it references other pods
            for pod_address in pod_addresses {
                info!("Checking pod {} for references", pod_address);
                let pod_refs = self.get_pod_references(&pod_address)?;

                for pod_ref in pod_refs {
                    // Extract the address from the ant:// URI
                    if let Some(ref_address) = pod_ref.strip_prefix("ant://") {
                        if !referenced_pods.contains(&ref_address.to_string()) {
                            referenced_pods.push(ref_address.to_string());
                        }
                    }
                }
            }

            // Download each referenced pod that we don't already have
            for ref_address in referenced_pods {
                info!("Processing referenced pod: {}", ref_address);

                // Check if we already have this pod locally
                if !self.data_store.address_is_pointer(&ref_address)? {
                    info!("Referenced pod {} not found locally, attempting to download", ref_address);

                    // Try to download the referenced pod
                    if let Err(e) = self.download_referenced_pod(&ref_address, current_depth + 1).await {
                        warn!("Failed to download referenced pod {}: {}", ref_address, e);
                        continue;
                    }
                } else {
                    // Update the depth if this pod is found at a shallower depth
                    self.update_pod_depth(&ref_address, current_depth + 1)?;
                }
            }
        }

        Ok(())
    }

    // Get all pod addresses at a specific depth
    fn get_pods_at_depth(&self, depth: u64) -> Result<Vec<String>, Error> {
        // Use the graph database to get pods at the specified depth
        let graph_pods = self.graph.get_pods_at_depth(depth)?;

        // If no pods found at this depth in the graph, check local pointers for depth 0
        if graph_pods.is_empty() && depth == 0 {
            // For depth 0, include all local pods that don't have a depth set yet
            let mut local_pods = Vec::new();
            for (address, _key) in self.key_store.get_pointers() {
                let address = address.trim();
                // Check if this pod has no depth set (returns u64::MAX)
                if let Ok(pod_depth) = self.graph.get_pod_depth(address) {
                    if pod_depth == u64::MAX || pod_depth == 0 {
                        local_pods.push(address.to_string());
                    }
                }
            }
            Ok(local_pods)
        } else {
            Ok(graph_pods)
        }
    }

    // Get the current depth of a pod from the graph database
    #[allow(dead_code)]
    fn get_pod_depth(&self, pod_address: &str) -> Result<u64, Error> {
        Ok(self.graph.get_pod_depth(pod_address)?)
    }

    // Get all pod references from a pod's graph data
    fn get_pod_references(&mut self, pod_address: &str) -> Result<Vec<String>, Error> {
        // Use the graph database to get pod references via SPARQL
        Ok(self.graph.get_pod_references(pod_address)?)
    }

    // Download a referenced pod from the network
    async fn download_referenced_pod(&mut self, pod_address: &str, depth: u64) -> Result<(), Error> {
        info!("Attempting to download referenced pod: {} at depth {}", pod_address, depth);

        // Try to analyze the address to see if it exists on the network
        let (address_type, create_mode) = self.get_address_type(pod_address).await?;

        if create_mode {
            warn!("Referenced pod {} does not exist on the network", pod_address);
            return Ok(()); // Not an error, just means the reference is invalid
        }

        match address_type {
            Analysis::Pointer(_) => {
                // This is a pointer (pod), download it
                let pointer_address = PointerAddress::from_hex(pod_address)?;
                let pointer = self.client.pointer_get(&pointer_address).await?;

                // Create local pointer file
                self.data_store.create_pointer_file(pod_address)?;
                self.data_store.update_pointer_target(pod_address, pointer.target().to_hex().as_str())?;
                self.data_store.update_pointer_count(pod_address, pointer.counter().into())?;

                // Download the scratchpad data
                let target = pointer.target();
                let target = match target {
                    PointerTarget::ScratchpadAddress(scratchpad_address) => scratchpad_address,
                    _ => {
                        error!("Pointer target is not a scratchpad address");
                        return Ok(());
                    }
                };

                // Create scratchpad file if it doesn't exist
                if !self.data_store.address_is_scratchpad(target.to_hex().as_str())? {
                    self.data_store.create_scratchpad_file(target.to_hex().as_str())?;
                }

                // Download the scratchpad data
                let scratchpad = self.client.scratchpad_get(target).await?;
                let data = scratchpad.encrypted_data();
                let data = String::from_utf8(data.to_vec())?;
                self.data_store.update_scratchpad_data(target.to_hex().as_str(), data.trim())?;

                // Load the downloaded pod data into the graph database
                // The data should be in TriG format, so we need to parse it and load it
                self.load_pod_into_graph(pod_address, data.trim())?;

                // Update the depth in the graph database
                self.update_pod_depth(pod_address, depth)?;

                info!("Successfully downloaded referenced pod: {}", pod_address);
            }
            _ => {
                warn!("Referenced address {} is not a pod pointer", pod_address);
            }
        }

        Ok(())
    }

    // Update the depth attribute of a pod in the graph database
    fn update_pod_depth(&mut self, pod_address: &str, depth: u64) -> Result<(), Error> {
        // Use the graph database to update the pod depth
        self.graph.update_pod_depth(pod_address, depth)?;
        Ok(())
    }

    // Load pod data into the graph database
    fn load_pod_into_graph(&mut self, pod_address: &str, pod_data: &str) -> Result<(), Error> {
        // The pod data should be in TriG format
        // Load it into the graph database using the Graph's method

        match self.graph.load_trig_data(pod_data) {
            Ok(_) => {
                info!("Successfully loaded pod {} data into graph database", pod_address);
            }
            Err(e) => {
                warn!("Failed to load pod {} data into graph database: {}", pod_address, e);
                // Don't fail the entire operation if graph loading fails
            }
        }

        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Helper function to create test components
    fn create_test_components() -> (DataStore, KeyStore, Graph, TempDir) {
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

    #[test]
    fn test_get_pods_at_depth() {
        let (_data_store, _key_store, mut graph, _temp_dir) = create_test_components();

        // Create a mock PodManager-like structure for testing
        let pod1 = "pod1_address";
        let pod2 = "pod2_address";
        let pod3 = "pod3_address";

        // Set up depths in the graph
        graph.update_pod_depth(pod1, 0).unwrap();
        graph.update_pod_depth(pod2, 1).unwrap();
        graph.update_pod_depth(pod3, 0).unwrap();

        // Test getting pods at different depths
        let pods_at_depth_0 = graph.get_pods_at_depth(0).unwrap();
        assert_eq!(pods_at_depth_0.len(), 2);
        assert!(pods_at_depth_0.contains(&pod1.to_string()));
        assert!(pods_at_depth_0.contains(&pod3.to_string()));

        let pods_at_depth_1 = graph.get_pods_at_depth(1).unwrap();
        assert_eq!(pods_at_depth_1.len(), 1);
        assert!(pods_at_depth_1.contains(&pod2.to_string()));
    }

    #[test]
    fn test_pod_reference_extraction() {
        let (_data_store, _key_store, mut graph, _temp_dir) = create_test_components();

        let pod_address = "test_pod";

        // Create test TriG data with references
        let trig_data = format!(r#"
            @prefix ant: <ant://> .
            <ant://{}> {{
                <ant://subject1> <ant://colonylib/vocabulary/0.1/predicate#references> <ant://referenced_pod1> .
                <ant://subject2> <ant://colonylib/vocabulary/0.1/predicate#links_to> <ant://referenced_pod2> .
                <ant://subject3> <ant://colonylib/vocabulary/0.1/predicate#name> "Test Name" .
            }}
        "#, pod_address);

        // Load the test data
        graph.load_trig_data(&trig_data).unwrap();

        // Get references
        let references = graph.get_pod_references(pod_address).unwrap();

        // Should find the referenced pods
        assert!(references.contains(&"ant://referenced_pod1".to_string()));
        assert!(references.contains(&"ant://referenced_pod2".to_string()));

        // Should not contain vocabulary URIs
        assert!(!references.iter().any(|r| r.contains("/vocabulary/")));
    }

    #[test]
    fn test_depth_update_logic() {
        let (_data_store, _key_store, mut graph, _temp_dir) = create_test_components();

        let pod_address = "test_depth_pod";

        // Initially no depth set (should return u64::MAX)
        let initial_depth = graph.get_pod_depth(pod_address).unwrap();
        assert_eq!(initial_depth, u64::MAX);

        // Set initial depth to 5
        graph.update_pod_depth(pod_address, 5).unwrap();
        assert_eq!(graph.get_pod_depth(pod_address).unwrap(), 5);

        // Try to set depth to 3 (should work since 3 < 5)
        graph.update_pod_depth(pod_address, 3).unwrap();
        assert_eq!(graph.get_pod_depth(pod_address).unwrap(), 3);

        // Try to set depth to 7 (should not change since 7 > 3)
        graph.update_pod_depth(pod_address, 7).unwrap();
        assert_eq!(graph.get_pod_depth(pod_address).unwrap(), 3);

        // Try to set depth to 1 (should work since 1 < 3)
        graph.update_pod_depth(pod_address, 1).unwrap();
        assert_eq!(graph.get_pod_depth(pod_address).unwrap(), 1);
    }

    #[test]
    fn test_data_store_integration() {
        let (data_store, _key_store, _graph, _temp_dir) = create_test_components();

        let pod_address = "integration_test_pod";
        let scratchpad_address = "integration_test_scratchpad";
        let test_data = "test pod data content";

        // Create pointer and scratchpad files
        data_store.create_pointer_file(pod_address).unwrap();
        data_store.create_scratchpad_file(scratchpad_address).unwrap();

        // Set up pointer to point to scratchpad
        data_store.update_pointer_target(pod_address, scratchpad_address).unwrap();
        data_store.update_scratchpad_data(scratchpad_address, test_data).unwrap();

        // Verify the setup
        assert!(data_store.address_is_pointer(pod_address).unwrap());
        assert!(data_store.address_is_scratchpad(scratchpad_address).unwrap());

        let target = data_store.get_pointer_target(pod_address).unwrap();
        assert_eq!(target, scratchpad_address);

        let retrieved_data = data_store.get_scratchpad_data(scratchpad_address).unwrap();
        assert_eq!(retrieved_data, test_data);
    }

    #[test]
    fn test_update_list_functionality() {
        let (data_store, _key_store, _graph, _temp_dir) = create_test_components();

        let addresses = vec!["addr1", "addr2", "addr3"];

        // Add addresses to update list
        for addr in &addresses {
            data_store.append_update_list(addr).unwrap();
        }

        // Verify update list file exists and contains all addresses
        let update_list_path = data_store.get_update_list_path();
        assert!(update_list_path.exists());

        let content = std::fs::read_to_string(update_list_path).unwrap();
        for addr in &addresses {
            assert!(content.contains(addr));
        }

        // Test duplicate prevention
        data_store.append_update_list("addr1").unwrap();
        let content = std::fs::read_to_string(data_store.get_update_list_path()).unwrap();
        let count = content.lines().filter(|line| *line == "addr1").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_graph_pod_entry_creation() {
        let (_data_store, _key_store, mut graph, _temp_dir) = create_test_components();

        let pod_address = "test_pod_entry";
        let scratchpad_address = "test_scratchpad_entry";

        // Create pod entry
        let trig_data = graph.add_pod_entry(pod_address, scratchpad_address).unwrap();

        // Verify the TriG data contains expected elements
        assert!(!trig_data.is_empty());
        // The function creates data about the scratchpad, not the pod address directly
        assert!(trig_data.contains(&format!("ant://{}", scratchpad_address)));
        // Check for the actual predicate URIs
        assert!(trig_data.contains("colonylib/vocabulary"));
        // Note: depth is stored in the default graph, not in the pod's named graph
        // so it won't appear in the TriG output for the specific pod graph

        // Verify that the depth was actually set by querying it directly
        let depth = graph.get_pod_depth(pod_address).unwrap();
        assert_eq!(depth, 0); // Initial depth should be 0
    }

    #[test]
    fn test_keystore_integration() {
        let (_data_store, mut key_store, _graph, _temp_dir) = create_test_components();

        // Test that keystore was created with the test mnemonic
        let expected_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        assert_eq!(key_store.get_seed_phrase(), expected_mnemonic);

        // Initially, no pointers or scratchpads should exist
        assert!(key_store.get_pointers().is_empty());
        assert!(key_store.get_scratchpads().is_empty());

        // Add keys and test that they exist
        key_store.add_pointer_key().unwrap();
        key_store.add_scratchpad_key().unwrap();

        assert!(!key_store.get_pointers().is_empty());
        assert!(!key_store.get_scratchpads().is_empty());
    }

    #[test]
    fn test_error_handling() {
        let (data_store, _key_store, graph, _temp_dir) = create_test_components();

        let non_existent_address = "non_existent_address";

        // Test DataStore error handling
        assert!(data_store.get_pointer_target(non_existent_address).is_err());
        assert!(data_store.get_scratchpad_data(non_existent_address).is_err());

        // Test Graph operations with non-existent data
        let depth = graph.get_pod_depth(non_existent_address).unwrap();
        assert_eq!(depth, u64::MAX); // Should return MAX for non-existent pods

        let references = graph.get_pod_references(non_existent_address).unwrap();
        assert!(references.is_empty()); // Should return empty vec for non-existent pods
    }

    #[test]
    fn test_search_functionality() {
        let (_data_store, _key_store, graph, _temp_dir) = create_test_components();

        // Add test data to the graph
        let pod_address = "test_search_pod";
        let pod_iri = format!("ant://{}", pod_address);

        // Add searchable content
        graph.put_quad(
            "ant://file1",
            "ant://colonylib/vocabulary/0.1/predicate#name",
            "Important Document",
            Some(&pod_iri)
        ).unwrap();

        graph.put_quad(
            "ant://file1",
            "ant://colonylib/vocabulary/0.1/predicate#description",
            "This document contains important information",
            Some(&pod_iri)
        ).unwrap();

        graph.put_quad(
            "ant://file2",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            "http://schema.org/MediaObject",
            Some(&pod_iri)
        ).unwrap();

        // Test search directly on graph (without network calls)
        let search_results = graph.search_content("important", Some(10)).unwrap();
        let parsed_results: serde_json::Value = serde_json::from_str(&search_results).unwrap();

        // Verify results structure
        assert!(parsed_results.get("results").is_some());
        let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
        assert!(bindings.len() > 0);

        // Test type search
        let type_results = graph.search_by_type("http://schema.org/MediaObject", Some(10)).unwrap();
        let parsed_type_results: serde_json::Value = serde_json::from_str(&type_results).unwrap();
        let type_bindings = parsed_type_results["results"]["bindings"].as_array().unwrap();
        assert_eq!(type_bindings.len(), 1);

        // Test predicate search
        let predicate_results = graph.search_by_predicate(
            "ant://colonylib/vocabulary/0.1/predicate#name",
            Some(10)
        ).unwrap();
        let parsed_predicate_results: serde_json::Value = serde_json::from_str(&predicate_results).unwrap();
        let predicate_bindings = parsed_predicate_results["results"]["bindings"].as_array().unwrap();
        assert_eq!(predicate_bindings.len(), 1);
    }

    #[test]
    fn test_structured_search_queries() {
        let (_data_store, _key_store, graph, _temp_dir) = create_test_components();

        // Add test data
        let pod_address = "test_structured_search";
        let pod_iri = format!("ant://{}", pod_address);

        graph.put_quad(
            "ant://media1",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            "http://schema.org/MediaObject",
            Some(&pod_iri)
        ).unwrap();

        graph.put_quad(
            "ant://media1",
            "ant://colonylib/vocabulary/0.1/predicate#name",
            "Test Video",
            Some(&pod_iri)
        ).unwrap();

        // Test text search directly on graph
        let text_results = graph.search_content("video", Some(10)).unwrap();
        let parsed_text_results: serde_json::Value = serde_json::from_str(&text_results).unwrap();
        let text_bindings = parsed_text_results["results"]["bindings"].as_array().unwrap();
        assert!(text_bindings.len() > 0);

        // Test type search directly on graph
        let type_results = graph.search_by_type("http://schema.org/MediaObject", Some(10)).unwrap();
        let parsed_type_results: serde_json::Value = serde_json::from_str(&type_results).unwrap();
        let type_bindings = parsed_type_results["results"]["bindings"].as_array().unwrap();
        assert_eq!(type_bindings.len(), 1);

        // Test predicate search directly on graph
        let predicate_results = graph.search_by_predicate(
            "ant://colonylib/vocabulary/0.1/predicate#name",
            Some(10)
        ).unwrap();
        let parsed_predicate_results: serde_json::Value = serde_json::from_str(&predicate_results).unwrap();
        let predicate_bindings = parsed_predicate_results["results"]["bindings"].as_array().unwrap();
        assert_eq!(predicate_bindings.len(), 1);

        // Test advanced search directly on graph
        let advanced_criteria = serde_json::json!({
            "text": "test",
            "type": "http://schema.org/MediaObject",
            "limit": 10
        });
        let advanced_results = graph.advanced_search(&advanced_criteria).unwrap();
        let parsed_advanced_results: serde_json::Value = serde_json::from_str(&advanced_results).unwrap();
        let advanced_bindings = parsed_advanced_results["results"]["bindings"].as_array().unwrap();
        assert!(advanced_bindings.len() > 0);
    }

    #[test]
    fn test_search_error_handling() {
        let (_data_store, _key_store, graph, _temp_dir) = create_test_components();

        // Test search with empty text (should return no results)
        let empty_results = graph.search_content("", Some(10)).unwrap();
        let parsed_empty_results: serde_json::Value = serde_json::from_str(&empty_results).unwrap();
        let empty_bindings = parsed_empty_results["results"]["bindings"].as_array().unwrap();
        assert_eq!(empty_bindings.len(), 0);

        // Test search with non-existent text
        let no_results = graph.search_content("nonexistent_text_12345", Some(10)).unwrap();
        let parsed_no_results: serde_json::Value = serde_json::from_str(&no_results).unwrap();
        let no_bindings = parsed_no_results["results"]["bindings"].as_array().unwrap();
        assert_eq!(no_bindings.len(), 0);

        // Test search by non-existent type
        let no_type_results = graph.search_by_type("http://example.com/NonExistentType", Some(10)).unwrap();
        let parsed_no_type_results: serde_json::Value = serde_json::from_str(&no_type_results).unwrap();
        let no_type_bindings = parsed_no_type_results["results"]["bindings"].as_array().unwrap();
        assert_eq!(no_type_bindings.len(), 0);

        // Test search by non-existent predicate
        let no_pred_results = graph.search_by_predicate("http://example.com/nonexistent", Some(10)).unwrap();
        let parsed_no_pred_results: serde_json::Value = serde_json::from_str(&no_pred_results).unwrap();
        let no_pred_bindings = parsed_no_pred_results["results"]["bindings"].as_array().unwrap();
        assert_eq!(no_pred_bindings.len(), 0);

        // Test advanced search with empty criteria
        let empty_criteria = serde_json::json!({});
        let empty_advanced_results = graph.advanced_search(&empty_criteria).unwrap();
        let parsed_empty_advanced: serde_json::Value = serde_json::from_str(&empty_advanced_results).unwrap();
        // Should return all triples (if any exist) since no filters are applied
        assert!(parsed_empty_advanced.get("results").is_some());
    }
}

