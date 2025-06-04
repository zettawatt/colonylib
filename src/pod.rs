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

    /// Creates a new PodManager instance with the provided components.
    ///
    /// This constructor initializes a PodManager that coordinates between the Autonomi network client,
    /// wallet for payments, local data storage, cryptographic key management, and RDF graph database.
    /// The PodManager serves as the main interface for pod operations including creation, modification,
    /// synchronization, and querying.
    ///
    /// # Parameters
    ///
    /// * `client` - An Autonomi network client for communicating with the decentralized network
    /// * `wallet` - A reference to a wallet for handling network transaction payments
    /// * `data_store` - A mutable reference to the local data storage system
    /// * `key_store` - A mutable reference to the cryptographic key management system
    /// * `graph` - A mutable reference to the RDF graph database for semantic data storage
    ///
    /// # Returns
    ///
    /// Returns `Ok(PodManager)` on successful initialization, or an `Error` if any component
    /// fails to initialize properly.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use autonomi::{Client, Wallet};
    /// use colonylib::{PodManager, DataStore, KeyStore, Graph};
    ///
    /// let client = Client::init_local().await?;
    /// let evm_network = client.evm_network();
    /// let wallet = &Wallet::new_from_private_key(evm_network.clone(), PRIVATE_KEY)?;
    /// let data_store = &mut DataStore::create()?;
    /// let key_store_file = data_store.get_keystore_path();
    /// let key_store: &mut KeyStore = if key_store_file.exists() {
    ///     let mut file = std::fs::File::open(key_store_file)?;
    ///     &mut KeyStore::from_file(&mut file, "password")?
    /// } else {
    ///     let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    ///     &mut KeyStore::from_mnemonic(mnemonic)?
    /// };
    /// let _ = key_store.set_wallet_key(PRIVATE_KEY.to_string())?;
    /// let graph_path = data_store.get_graph_path();
    /// let graph = &mut Graph::open(&graph_path)?;
    /// let pod_manager = PodManager::new(client, wallet, data_store, key_store, graph).await?;
    /// ```
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

    /// Performs a comprehensive search across all pod data using various search strategies.
    ///
    /// This function provides a flexible search interface that supports multiple search types
    /// including text search, type-based search, predicate-based search, and advanced SPARQL queries.
    /// The search operates across all loaded pods in the graph database and returns enhanced
    /// results with metadata.
    ///
    /// # Parameters
    ///
    /// * `query` - A JSON value containing the search parameters. Can be either:
    ///   - A simple string for basic text search
    ///   - A structured object with specific search type and parameters
    ///
    /// # Supported Query Types
    ///
    /// ## Text Search
    /// ```json
    /// {
    ///   "type": "text",
    ///   "text": "search term",
    ///   "limit": 50
    /// }
    /// ```
    ///
    /// ## Type-based Search
    /// ```json
    /// {
    ///   "type": "by_type",
    ///   "type_uri": "http://example.org/MyType",
    ///   "limit": 100
    /// }
    /// ```
    ///
    /// ## Predicate-based Search
    /// ```json
    /// {
    ///   "type": "by_predicate",
    ///   "predicate_uri": "http://example.org/hasProperty",
    ///   "limit": 25
    /// }
    /// ```
    ///
    /// ## Advanced Search
    /// ```json
    /// {
    ///   "type": "advanced",
    ///   "sparql": "SELECT ?s ?p ?o WHERE { ?s ?p ?o }"
    /// }
    /// ```
    ///
    /// # Returns
    ///
    /// Returns a JSON object containing:
    /// - `sparql_results` - The raw SPARQL query results
    /// - `result_count` - Number of results found
    /// - `pods_found` - Array of pod addresses that contain matching data
    /// - `search_timestamp` - ISO 8601 timestamp of when the search was performed
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The query format is invalid
    /// - Required parameters are missing for the specified search type
    /// - The underlying graph database query fails
    /// - JSON parsing of results fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// use serde_json::{json, Value};
    ///
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // Simple text search
    /// let results = pod_manager.search(json!("my search term")).await?;
    ///
    /// // Structured search for specific type
    /// let type_search = json!({
    ///     "type": "by_type",
    ///     "type_uri": "http://schema.org/Person",
    ///     "limit": 10
    /// });
    /// let results = pod_manager.search(type_search).await?;
    ///
    /// // Advanced SPARQL query
    /// let advanced_search = json!({
    ///     "type": "advanced",
    ///     "sparql": "SELECT ?name WHERE { ?person <http://schema.org/name> ?name }"
    /// });
    /// let results = pod_manager.search(advanced_search).await?;
    /// # Ok(())
    /// # }
    /// ```
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

    /// Adds, modifies, or removes semantic data for a specific subject within a pod.
    ///
    /// This function updates the RDF graph data associated with a subject (identified by its Autonomi address)
    /// within a specific pod. The data is stored in the pod's graph entry in the database and automatically
    /// synchronized to the associated scratchpad(s) for network storage. The operation is queued
    /// for upload to the Autonomi network.
    ///
    /// # Parameters
    ///
    /// * `pod_address` - The hexadecimal Autonomi address of the pod to update
    /// * `subject_address` - The hexadecimal Autonomi address of the object whose metadata is being updated
    /// * `subject_data` - JSON-LD structured RDF data describing the subject. Use empty string to remove data.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful update, or an `Error` if:
    /// - The pod address is invalid or doesn't exist
    /// - The subject data is malformed
    /// - Graph database update fails
    /// - Local storage update fails
    ///
    /// # Side Effects
    ///
    /// - Updates the local graph database with the new subject data
    /// - Writes updated graph data to associated scratchpad files
    /// - Adds the pod and scratchpad addresses to the upload queue
    /// - The changes will be uploaded to the network on the next `upload_all()` call
    ///
    /// # Example
    ///
    /// ```ignore
    /// use serde_json::json;
    ///
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// let pod_address = "80e79010a13e7eee779f799d99a20b418436828269b18192d92940bc9ddbfe295a7e1823d7bff75c59cbacbdea101a0d"; // Pod Autonomi address
    /// let subject_address = "c859818c623ce4fc0899c2ab43061b19caa0b0598eec35ef309dbe50c8af8d59"; // Subject Autonomi address
    ///
    /// // Add metadata for a document
    /// let metadata = json!({
    ///     "@context": "http://schema.org/",
    ///     "@type": "TextDigitalDocument",
    ///     "name": "Important Document",
    ///     "author": "John Doe",
    ///     "dateCreated": "2024-01-15",
    ///     "description": "A document containing important information"
    /// }).to_string();
    ///
    /// pod_manager.put_subject_data(pod_address, subject_address, &metadata).await?;
    ///
    /// // Remove metadata by providing empty data
    /// pod_manager.put_subject_data(pod_address, subject_address, "").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`get_subject_data`] - Retrieve data for a specific subject
    /// - [`upload_all`] - Upload pending changes to the network
    /// - [`search`] - Search for subjects across pods
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

    /// Retrieves all semantic data associated with a specific subject across all pods.
    ///
    /// This function queries the graph database to find all RDF triples where the specified
    /// subject address appears as the subject. It returns the data in JSON format, aggregating
    /// information from all pods that contain data about this subject.
    ///
    /// # Parameters
    ///
    /// * `subject_address` - The Autonomi address of the object to retrieve data for
    ///
    /// # Returns
    ///
    /// Returns a JSON string containing all metadata associated with the subject, or an `Error` if:
    /// - The subject address is invalid
    /// - The graph database query fails
    /// - JSON serialization fails
    ///
    /// The returned JSON follows the SPARQL JSON Results format with bindings for each
    /// predicate-object pair associated with the subject.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// let subject_address = "c859818c623ce4fc0899c2ab43061b19caa0b0598eec35ef309dbe50c8af8d59";
    ///
    /// // Retrieve all metadata for the subject
    /// let metadata_json = pod_manager.get_subject_data(subject_address).await?;
    ///
    /// // Parse the JSON to work with the data
    /// let metadata: serde_json::Value = serde_json::from_str(&metadata_json)?;
    ///
    /// // Access the SPARQL results
    /// if let Some(bindings) = metadata["results"]["bindings"].as_array() {
    ///     for binding in bindings {
    ///         if let (Some(predicate), Some(object)) = (
    ///             binding["predicate"]["value"].as_str(),
    ///             binding["object"]["value"].as_str()
    ///         ) {
    ///             println!("Property: {}, Value: {}", predicate, object);
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`put_subject_data`] - Add or update data for a subject
    /// - [`search`] - Search for subjects with specific criteria
    pub async fn get_subject_data(&mut self, subject_address: &str) -> Result<String, Error> {
        // Perform a SPARQL query with the Autonomi object address and return the metadata as JSON results
        let json_data = self.graph.get_subject_data(subject_address)?;

        Ok(json_data)
    }

    fn get_pod_scratchpads(&self, address: &str) -> Result<Option<Vec<String>>, Error> {
        // Get all scratchpad addresses for this pod from the graph database
        match self.graph.get_pod_scratchpads(address) {
            Ok(scratchpads) => {
                if scratchpads.is_empty() {
                    // Fallback to the pointer target if no scratchpads found in graph
                    let target = self.data_store.get_pointer_target(address)?;
                    Ok(Some(vec![target]))
                } else {
                    Ok(Some(scratchpads))
                }
            }
            Err(_) => {
                // Fallback to the pointer target if graph query fails
                let target = self.data_store.get_pointer_target(address)?;
                Ok(Some(vec![target]))
            }
        }
    }

    ///////////////////////////////////////////
    // Local data operations
    ///////////////////////////////////////////
    

    /// Creates a new pod with the specified name in the local data store.
    ///
    /// This function creates a complete pod structure including:
    /// - A new pointer address for the pod
    /// - A new scratchpad address for data storage
    /// - Initial graph data with pod metadata
    /// - Local files for both pointer and scratchpad
    /// - Adds the addresses to the upload queue
    ///
    /// The pod will be ready for use immediately and will be uploaded to the Autonomi network
    /// on the next call to `upload_all()`.
    ///
    /// # Parameters
    ///
    /// * `pod_name` - A human-readable name for the pod (used in metadata)
    ///
    /// # Returns
    ///
    /// Returns `Ok((pointer_address, scratchpad_address))` containing:
    /// - `pointer_address` - The hexadecimal address of the pod's pointer
    /// - `scratchpad_address` - The hexadecimal address of the pod's primary scratchpad
    ///
    /// Returns an `Error` if:
    /// - Key generation fails
    /// - File creation fails
    /// - Graph database update fails
    /// - Local storage operations fail
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // Create a new pod for storing document metadata
    /// let (pod_address, scratchpad_address) = pod_manager.add_pod("My Documents").await?;
    ///
    /// println!("Created pod at address: {}", pod_address);
    /// println!("Primary scratchpad at: {}", scratchpad_address);
    ///
    /// // The pod is now ready to store data
    /// let subject_data = r#"{
    ///     "@context": "http://schema.org/",
    ///     "@type": "Collection",
    ///     "name": "My Documents",
    ///     "description": "A collection of important documents"
    /// }"#;
    ///
    /// pod_manager.put_subject_data(&pod_address, &pod_address, subject_data).await?;
    ///
    /// // Upload the new pod to the network
    /// pod_manager.upload_all().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`add_pod_ref`] - Add a reference to another pod
    /// - [`upload_all`] - Upload the new pod to the network
    /// - [`put_subject_data`] - Add data to the pod
    pub async fn add_pod(&mut self, pod_name: &str) -> Result<(String,String), Error> {
        let scratchpad_address = self.add_scratchpad().await?;
        let scratchpad_address = scratchpad_address.to_hex();
        let scratchpad_address = scratchpad_address.as_str();
        let pointer_address = self.add_pointer().await?;
        let pointer_address = pointer_address.to_hex();
        let pointer_address = pointer_address.as_str();

        // Add the scratchpad address to the pointer file
        let _ = self.data_store.update_pointer_target(pointer_address, scratchpad_address)?;

        // Add initial data to the scratchpad
        let pod_data = self.graph.add_pod_entry(pod_name, pointer_address, scratchpad_address)?;
        let _ = self.data_store.update_scratchpad_data(scratchpad_address, pod_data.as_str())?;
        Ok((pointer_address.to_string(), scratchpad_address.to_string()))
    }

    /// Adds a reference from one pod to another pod in the graph database.
    ///
    /// This function creates a semantic link between two pods, allowing for the creation
    /// of pod networks and hierarchies. The reference is stored in the graph database
    /// and will be included when the referencing pod is uploaded to the network.
    /// Referenced pods can be discovered and downloaded automatically using `refresh_ref()`.
    ///
    /// # Parameters
    ///
    /// * `pod_address` - The hexadecimal Autonomi address of the pod that will store the referenced pod address
    /// * `pod_ref_address` - The hexadecimal Autonomi address of the pod being referenced
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if:
    /// - Either pod address is invalid
    /// - The graph database update fails
    /// - The referencing pod doesn't exist locally
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // Create a main pod and a sub-pod
    /// let (main_pod, _) = pod_manager.add_pod("Main Collection").await?;
    /// let (sub_pod, _) = pod_manager.add_pod("Sub Collection").await?;
    ///
    /// // Create a reference from main pod to sub pod
    /// pod_manager.add_pod_ref(&main_pod, &sub_pod)?;
    ///
    /// // The reference will be included when uploading the main pod
    /// pod_manager.upload_all().await?;
    ///
    /// // Later, when refreshing with references, the sub pod will be discovered
    /// pod_manager.refresh_ref(2).await?; // Refresh up to depth 2
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`add_pod`] - Create a new pod
    /// - [`refresh_ref`] - Download referenced pods from the network
    /// - [`upload_all`] - Upload pod references to the network
    pub fn add_pod_ref(&mut self, pod_address: &str, pod_ref_address: &str) -> Result<(), Error> {
        // Add the pointer address to the graph
        let graph = self.graph.add_pod_ref_entry(pod_address, pod_ref_address)?;
        
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
                    // The unwraps in this section are OK because we're just making some default objects for analysis purposes. There aren't any unknowns here
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
    
    /// Uploads all pending changes to the Autonomi network.
    ///
    /// This function processes the update queue and uploads all modified pods and scratchpads
    /// to the Autonomi network. It handles both creating new network objects and updating
    /// existing ones based on their current state. The function automatically determines
    /// whether each address needs to be created or updated.
    ///
    /// # Process
    ///
    /// 1. Reads the update list containing addresses that need uploading
    /// 2. For each address, determines if it's a pointer or scratchpad
    /// 3. Checks if the address exists on the network (create vs update)
    /// 4. Performs the appropriate network operation
    /// 5. Clears the update list upon successful completion
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful upload of all items, or an `Error` if:
    /// - Network communication fails
    /// - Payment processing fails
    /// - Local file operations fail
    /// - Address analysis fails
    ///
    /// # Network Costs
    ///
    /// This operation incurs network costs for:
    /// - Creating new pointers and scratchpads (used to construct pods)
    /// - Adding data to an existing pod that causes a new scratchpad to be required (each scratchpad's max size is 4MB)
    ///
    /// Costs are automatically paid using the configured wallet. Updates to existing pod components are free.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // Create a new pod
    /// let (pod_address, _) = pod_manager.add_pod("My New Pod").await?;
    ///
    /// // Add some data to the pod for the subject
    /// let subject_address = "c859818c623ce4fc0899c2ab43061b19caa0b0598eec35ef309dbe50c8af8d59";
    /// let metadata = r#"{
    ///     "@context": "http://schema.org/",
    ///     "@type": "Dataset",
    ///     "name": "Research Data"
    /// }"#;
    /// pod_manager.put_subject_data(&pod_address, subject_address, metadata).await?;
    ///
    /// // Upload all changes to the network
    /// pod_manager.upload_all().await?;
    ///
    /// println!("All changes uploaded successfully!");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`add_pod`] - Creates pods that need uploading
    /// - [`put_subject_data`] - Modifies pods that need uploading
    /// - [`refresh_cache`] - Downloads updates from the network
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

    /// Refreshes the local cache by discovering and downloading user created pods from the Autonomi network.
    ///
    /// This function performs a comprehensive refresh of the local pod cache by:
    /// 1. Discovering new keys that may have been created on different devices
    /// 2. Downloading any new or updated pods associated with known keys
    /// 3. Updating the local graph database with fresh pod data
    /// 4. Synchronizing pointer and scratchpad files that make up the pods
    ///
    /// The function automatically discovers pods that may have been created on other devices
    /// using the same key derivation, ensuring synchronization across multiple clients.
    ///
    /// # Process
    ///
    /// 1. **Key Discovery**: Checks the next few derived keys for network activity
    /// 2. **Pod Discovery**: Downloads any new pods found at discovered addresses
    /// 3. **Update Check**: Compares local and remote versions of known pods
    /// 4. **Data Sync**: Downloads updated pod data and updates the graph database
    /// 5. **Depth Setting**: Marks all discovered pods with depth 0 (local pods)
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful refresh, or an `Error` if:
    /// - Network communication fails
    /// - Key derivation fails
    /// - Local file operations fail
    /// - Graph database updates fail
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // Refresh the cache to discover any new or updated local pods
    /// pod_manager.refresh_cache().await?;
    ///
    /// // The cache is now up to date with the network
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Performance Notes
    ///
    /// - This operation may take time proportional to the number of pods
    /// - Network requests are made for each pod to check for updates
    /// - Consider calling this periodically rather than on every operation
    ///
    /// # Related Functions
    ///
    /// - [`refresh_ref`] - Refresh cache including pod references
    /// - [`upload_all`] - Upload local changes before refreshing
    /// - [`search`] - Search across refreshed pod data
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

                // Load the newly discovered pod data into the graph database
                info!("Loading newly discovered pod into graph database: {}", address);
                if !data.trim().is_empty() {
                    if let Err(e) = self.load_pod_into_graph(address, data.trim()) {
                        warn!("Failed to load newly discovered pod data into graph for {}: {}", address, e);
                    }
                }

                // Set the depth attribute to 0 (local pod)
                if let Err(e) = self.update_pod_depth(address, 0) {
                    warn!("Failed to update pod depth for newly discovered pod {}: {}", address, e);
                }

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

                // Update graph database with newly discovered or updated pod data
                info!("Updating graph database for pod: {}", address);

                // Clear existing pod graph if it exists
                if let Err(e) = self.graph.clear_pod_graph(address) {
                    warn!("Failed to clear existing pod graph for {}: {}", address, e);
                }

                // Get all scratchpads that are part of this pod
                let scratchpad_addresses = self.get_pod_scratchpads(address)?;
                if let Some(addresses) = scratchpad_addresses {
                    // Collect all scratchpad data
                    let mut combined_data = String::new();

                    for scratchpad_addr in addresses {
                        match self.data_store.get_scratchpad_data(scratchpad_addr.trim()) {
                            Ok(scratchpad_data) => {
                                if !scratchpad_data.trim().is_empty() {
                                    combined_data.push_str(scratchpad_data.trim());
                                    combined_data.push('\n');
                                }
                            }
                            Err(e) => {
                                warn!("Failed to read scratchpad data for {}: {}", scratchpad_addr, e);
                            }
                        }
                    }

                    // Load the combined pod data into the graph database
                    if !combined_data.trim().is_empty() {
                        if let Err(e) = self.load_pod_into_graph(address, combined_data.trim()) {
                            warn!("Failed to load pod data into graph for {}: {}", address, e);
                        }
                    }
                }

                // Set the depth attribute to 0 (local pod)
                if let Err(e) = self.update_pod_depth(address, 0) {
                    warn!("Failed to update pod depth for {}: {}", address, e);
                }

                info!("Successfully updated graph database for pod: {}", address);
            } else {
                info!("Pointer is up to date");
            }
        }
        Ok(())
    }
 
    /// Refreshes the pod cache including referenced pods up to a specified depth.
    ///
    /// This function extends `refresh_cache()` by also discovering and downloading pods
    /// that are referenced by local pods, creating a network of interconnected pods.
    /// It processes pod references iteratively up to the specified depth to avoid
    /// taking excessive time.
    ///
    /// # Parameters
    ///
    /// * `depth` - Maximum depth of pod references to follow:
    ///   - `0`: Only refresh local pods (equivalent to `refresh_cache()`)
    ///   - `1`: Include pods directly referenced by local pods
    ///   - `2`: Include pods referenced by referenced pods, etc.
    ///
    /// # Process
    ///
    /// 1. **Initial Refresh**: Calls `refresh_cache()` to update local pods
    /// 2. **Iterative Processing**: For each depth level:
    ///    - Gets all pods at the current depth
    ///    - Extracts pod references from their graph data
    ///    - Downloads referenced pods that don't exist locally
    ///    - Updates depth metadata for discovered pods
    /// 3. **Depth Management**: Assigns appropriate depth values to maintain hierarchy
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful refresh, or an `Error` if:
    /// - Network communication fails
    /// - Referenced pods cannot be downloaded
    /// - Graph database operations fail
    /// - Local storage operations fail
    ///
    /// # Network Costs
    ///
    /// This operation is free in terms of cost, but can take a significant amount of time as it may download
    /// many referenced pods. Consider the depth parameter carefully:
    /// - Higher depths exponentially increase potential downloads
    /// - Referenced pods may reference many other pods
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // Refresh with depth 1 to include directly referenced pods
    /// pod_manager.refresh_ref(1).await?;
    ///
    /// // Search across all local and referenced pods
    /// let results = pod_manager.search(serde_json::json!({
    ///     "type": "text",
    ///     "text": "research data",
    ///     "limit": 100
    /// })).await?;
    ///
    /// println!("Found data across {} pods", results["pods_found"].as_array().unwrap().len());
    ///
    /// // Refresh with deeper references (use cautiously)
    /// pod_manager.refresh_ref(8).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`refresh_cache`] - Refresh only local pods
    /// - [`add_pod_ref`] - Create pod references
    /// - [`search`] - Search across all cached pods
    pub async fn refresh_ref(&mut self, depth: u64) -> Result<(), Error> {
        let _ = self.refresh_cache().await?;

        // Process pods iteratively up to the specified depth to avoid async recursion
        for current_depth in 0..=depth {
            info!("Processing pod references at depth {}", current_depth);

            // Get all pods at the current depth
            let pod_addresses = self.get_pods_at_depth(current_depth)?;
            let mut referenced_pods = Vec::new();

            // Walk through each pod graph and check if it references other pods
            for pod_address in pod_addresses {
                info!("Checking pod {} for references", pod_address);
                let pod_refs = self.get_pod_references(&pod_address)?;

                for pod_ref in pod_refs {
                    referenced_pods.push(pod_ref.to_string());
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
        Ok(graph_pods)
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
                
                // Check if we already have this pod locally
                let pod_exists = self.data_store.address_is_pointer(pod_address)?;
                let should_download = if pod_exists {
                    // Check if the remote version is newer than our local version
                    let local_pointer_count = self.data_store.get_pointer_count(pod_address)?;
                    let remote_counter = pointer.counter() as u64;
                    if remote_counter > local_pointer_count {
                        info!("Remote pod is newer (counter: {} > {}), downloading update", 
                              remote_counter, local_pointer_count);
                        true
                    } else {
                        info!("Local pod is up to date (counter: {} >= {}), skipping download", 
                              local_pointer_count, remote_counter);
                        false
                    }
                } else {
                    // Pod doesn't exist locally, download it
                    info!("Pod doesn't exist locally, downloading for the first time");
                    // Create local pointer file
                    self.data_store.create_pointer_file(pod_address)?;
                    true
                };

                if should_download {
                    // Update pointer information
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
                    self.load_pod_into_graph(pod_address, data.trim())?;

                    info!("Successfully downloaded referenced pod: {}", pod_address);
                }

                // Always update the depth in the graph database, even if we didn't download
                self.update_pod_depth(pod_address, depth)?;
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

        match self.graph.load_pod_into_graph(pod_address, pod_data) {
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



