use autonomi;
use autonomi::client::payment::PaymentOption;
use autonomi::client::pointer::{Pointer, PointerAddress, PointerError, PointerTarget};
use autonomi::client::scratchpad::{Scratchpad, ScratchpadAddress, ScratchpadError};
use autonomi::client::{ConnectError, GetError};
use autonomi::{AddressParseError, Bytes, Chunk, Client, SecretKey, Wallet};

use alloc::string::FromUtf8Error;
use autonomi::client::analyze::{Analysis, AnalysisError};
use blsttc::Error as BlsttcError;
use chrono::Utc;
use futures::future::{join_all, try_join_all};
use serde::{Deserialize, Serialize};
use serde_json::{Error as SerdeError, Value};
use std::collections::HashMap;
use std::fmt;
use std::io::Error as IoError;
use thiserror;
use tracing::{debug, error, info, warn};

use crate::DataStore;
use crate::Graph;
use crate::KeyStore;
use crate::data::Error as DataStoreError;
use crate::graph::Error as GraphError;
use crate::key::Error as KeyStoreError;

/// Structure representing the removal section of the update list
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct RemovalSection {
    /// Pointer addresses to be removed (updated to point to themselves)
    #[serde(default)]
    pub pointers: Vec<String>,
    /// Scratchpad addresses to be removed (updated with empty data)
    #[serde(default)]
    pub scratchpads: Vec<String>,
}

/// Structure representing the complete update list in JSON format
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct UpdateList {
    /// Items to be removed from the network
    #[serde(default)]
    pub remove: RemovalSection,
    /// Pod addresses mapped to their associated scratchpad addresses for upload
    #[serde(default)]
    pub pods: std::collections::HashMap<String, Vec<String>>,
}

/// Batch operation types for concurrent network operations
#[derive(Debug, Clone)]
pub enum BatchOperation {
    PointerGet { address: String },
    PointerPut { address: String, target: String },
    PointerUpdate { address: String, target: String },
    ScratchpadGet { address: String },
    ScratchpadPut { address: String, data: String },
    ScratchpadUpdate { address: String, data: String },
}

/// Results from batch operations
#[derive(Debug, Clone)]
pub enum BatchResult {
    PointerGetResult {
        address: String,
        success: bool,
        data: Option<String>,
    },
    PointerPutResult {
        address: String,
        success: bool,
    },
    PointerUpdateResult {
        address: String,
        success: bool,
    },
    ScratchpadGetResult {
        address: String,
        success: bool,
        data: Option<String>,
    },
    ScratchpadPutResult {
        address: String,
        success: bool,
    },
    ScratchpadUpdateResult {
        address: String,
        success: bool,
    },
}

/// Preprocessed batch of operations ready for concurrent execution
#[derive(Debug, Clone, Default)]
pub struct NetworkBatch {
    pub pointer_gets: Vec<String>,
    pub pointer_puts: Vec<(String, String)>, // (address, target)
    pub pointer_updates: Vec<(String, String)>, // (address, target)
    pub scratchpad_gets: Vec<String>,
    pub scratchpad_puts: Vec<(String, String)>, // (address, data)
    pub scratchpad_updates: Vec<(String, String)>, // (address, data)
}

use crate::graph;

// Error handling
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Connect(#[from] ConnectError),
    #[error(transparent)]
    Pointer(Box<PointerError>),
    #[error(transparent)]
    Scratchpad(Box<ScratchpadError>),
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
    #[error("{0}")]
    Pod(String),
}

impl From<PointerError> for Error {
    fn from(err: PointerError) -> Self {
        Error::Pointer(Box::new(err))
    }
}

impl From<ScratchpadError> for Error {
    fn from(err: ScratchpadError) -> Self {
        Error::Scratchpad(Box::new(err))
    }
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
    Pod(String),
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
            Self::Pod(_) => ErrorKind::Pod(error_message),
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
    /// let _ = key_store.add_wallet_key("main", PRIVATE_KEY)?;
    /// let graph_path = data_store.get_graph_path();
    /// let graph = &mut Graph::open(&graph_path)?;
    /// let pod_manager = PodManager::new(client, wallet, data_store, key_store, graph).await?;
    /// ```
    pub async fn new(
        client: Client,
        wallet: &'a Wallet,
        data_store: &'a mut DataStore,
        key_store: &'a mut KeyStore,
        graph: &'a mut Graph,
    ) -> Result<Self, Error> {
        Ok(Self {
            client,
            wallet,
            data_store,
            key_store,
            graph,
        })
    }

    // Create a new pointer key, make sure it is empty, and add it to the key store
    async fn create_pointer_key(&mut self) -> Result<SecretKey, Error> {
        // Derive a new key
        info!("Deriving or using a free key");
        let (pubkey, key) = self.key_store.add_pointer_key()?;

        // If the address is being freed, unset the FREE attribute in the configuration graph
        let configuration_address = self.key_store.get_configuration_address()?;
        let configuration_address = configuration_address.as_str();
        self.graph
            .use_free_pointer(pubkey.as_str(), configuration_address)?;

        info!("New key: {}", key);
        let derived_key: SecretKey = SecretKey::from_hex(key.trim())?;
        Ok(derived_key)
    }

    // Create a new scratchpad key, make sure it is empty, and add it to the key store
    async fn create_scratchpad_key(&mut self) -> Result<SecretKey, Error> {
        // Derive a new key
        info!("Deriving or using a free key");
        let (pubkey, key) = self.key_store.add_scratchpad_key()?;

        // If the address is being freed, unset the FREE attribute in the configuration graph
        let configuration_address = self.key_store.get_configuration_address()?;
        let configuration_address = configuration_address.as_str();
        self.graph
            .use_free_scratchpad(pubkey.as_str(), configuration_address)?;

        info!("New key: {}", key);
        let derived_key: SecretKey = SecretKey::from_hex(key.trim())?;
        Ok(derived_key)
    }

    ///////////////////////////////////////////
    // Graph operations
    ///////////////////////////////////////////

    /// Performs a comprehensive search across all pod data using various search strategies.
    ///
    /// This function provides a flexible search interface that supports multiple search types
    /// including text search, type-based search, predicate-based search, browse functionality,
    /// and advanced SPARQL queries. The search operates across all loaded pods in the graph
    /// database and returns enhanced results with metadata.
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
    /// ## Browse
    /// ```json
    /// {
    ///   "type": "browse",
    ///   "limit": 100
    /// }
    /// ```
    /// Browse all subjects with their name, type, and description, ordered by pod depth.
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
    /// // Browse all subjects ordered by pod depth
    /// let browse_search = json!({
    ///     "type": "browse",
    ///     "limit": 50
    /// });
    /// let results = pod_manager.search(browse_search).await?;
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
                            return Ok(Value::Object(serde_json::Map::from_iter([(
                                "error".to_string(),
                                Value::String(
                                    "Missing 'text' parameter for text search".to_string(),
                                ),
                            )])));
                        }
                    }
                    "by_type" => {
                        // Search by RDF type
                        if let Some(type_uri) = query_obj.get("type_uri").and_then(|v| v.as_str()) {
                            let limit = query_obj.get("limit").and_then(|v| v.as_u64());
                            self.graph.search_by_type(type_uri, limit)?
                        } else {
                            return Ok(Value::Object(serde_json::Map::from_iter([(
                                "error".to_string(),
                                Value::String(
                                    "Missing 'type_uri' parameter for type search".to_string(),
                                ),
                            )])));
                        }
                    }
                    "by_predicate" => {
                        // Search by predicate
                        if let Some(predicate_uri) =
                            query_obj.get("predicate_uri").and_then(|v| v.as_str())
                        {
                            let limit = query_obj.get("limit").and_then(|v| v.as_u64());
                            self.graph.search_by_predicate(predicate_uri, limit)?
                        } else {
                            return Ok(Value::Object(serde_json::Map::from_iter([(
                                "error".to_string(),
                                Value::String(
                                    "Missing 'predicate_uri' parameter for predicate search"
                                        .to_string(),
                                ),
                            )])));
                        }
                    }
                    "advanced" => {
                        // Advanced search with multiple criteria
                        if let Some(sparql) = query_obj.get("sparql").and_then(|v| v.as_str()) {
                            self.graph.advanced_search(sparql)?
                        } else {
                            return Ok(Value::Object(serde_json::Map::from_iter([(
                                "error".to_string(),
                                Value::String(
                                    "Missing 'sparql' parameter for advanced search".to_string(),
                                ),
                            )])));
                        }
                    }
                    "browse" => {
                        // Browse all subjects ordered by pod depth
                        let limit = query_obj.get("limit").and_then(|v| v.as_u64());
                        self.graph.browse(limit)?
                    }
                    _ => {
                        return Ok(Value::Object(serde_json::Map::from_iter([(
                            "error".to_string(),
                            Value::String(format!("Unknown search type: {search_type}")),
                        )])));
                    }
                }
            } else {
                // No explicit type, treat as advanced search
                return Ok(Value::Object(serde_json::Map::from_iter([(
                    "error".to_string(),
                    Value::String("No search type provided: none".to_string()),
                )])));
            }
        } else if let Some(text) = query.as_str() {
            // Simple text search if query is just a string
            self.graph.search_content(text, Some(50))?
        } else {
            return Ok(Value::Object(serde_json::Map::from_iter([(
                "error".to_string(),
                Value::String("Invalid query format. Expected object or string.".to_string()),
            )])));
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
            enhanced.insert(
                "search_timestamp".to_string(),
                Value::String(chrono::Utc::now().to_rfc3339()),
            );

            // Count results
            if let Some(bindings) = results_obj
                .get("results")
                .and_then(|r| r.get("bindings"))
                .and_then(|b| b.as_array())
            {
                enhanced.insert(
                    "result_count".to_string(),
                    Value::Number(serde_json::Number::from(bindings.len())),
                );

                // Extract unique graphs (pods) from results
                let mut unique_graphs = std::collections::HashSet::new();
                for binding in bindings {
                    if let Some(graph_value) = binding
                        .get("graph")
                        .and_then(|g| g.get("value"))
                        .and_then(|v| v.as_str())
                    {
                        unique_graphs.insert(graph_value.to_string());
                    }
                }

                let graphs_vec: Vec<Value> = unique_graphs.into_iter().map(Value::String).collect();
                enhanced.insert("pods_found".to_string(), Value::Array(graphs_vec));
            } else {
                enhanced.insert(
                    "result_count".to_string(),
                    Value::Number(serde_json::Number::from(0)),
                );
                enhanced.insert("pods_found".to_string(), Value::Array(vec![]));
            }
        } else {
            // If results is not an object, just wrap it
            enhanced.insert("sparql_results".to_string(), results);
            enhanced.insert(
                "result_count".to_string(),
                Value::Number(serde_json::Number::from(0)),
            );
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
    pub async fn put_subject_data(
        &mut self,
        pod_address: &str,
        subject_address: &str,
        subject_data: &str,
    ) -> Result<(), Error> {
        let pod_address = self.graph.check_pod_exists(pod_address)?;
        let pod_address = pod_address.trim();
        let configuration_address = self.key_store.get_configuration_address()?;
        let configuration_address = configuration_address.as_str();

        // Inject the JSON data into the graph using the pod address as the named graph
        // And return the resulting graph data as a TriG formatted byte vector
        let (graph, configuration) = self.graph.put_subject_data(
            pod_address,
            subject_address,
            configuration_address,
            subject_data,
        )?;

        // Process the pod data with proper scratchpad management
        self.process_pod_data(pod_address, graph).await?;
        // Update the configuration graph with the updated key count
        let num_keys = self.key_store.get_num_keys();
        self.graph
            .update_key_count(configuration_address, num_keys)?;
        self.process_pod_data(configuration_address, configuration)
            .await?;

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

    pub fn get_pod_scratchpads(&self, address: &str) -> Result<Option<Vec<String>>, Error> {
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

    async fn remove_pod_data(
        &mut self,
        pod_address: &str,
        pod_scratchpads: Vec<String>,
    ) -> Result<(), Error> {
        // Remove the pod address from the key store pointers list
        self.key_store.remove_pointer_key(pod_address)?;
        // Remove the pod scratchpads from the key store scratchpads list
        for scratchpad in pod_scratchpads.clone() {
            self.key_store.remove_scratchpad_key(scratchpad.trim())?;
        }

        // Remove each scratchpad file from the data store
        for scratchpad in pod_scratchpads.clone() {
            self.data_store.remove_scratchpad_file(scratchpad.trim())?;
        }
        // Remove the pod pointer file from the data store
        self.data_store.remove_pointer_file(pod_address)?;

        // Mark the removal of the pod pointer and scratchpads for the next upload_all operation
        self.data_store
            .append_removal_list(pod_address, "pointer")?;
        for scratchpad in pod_scratchpads {
            self.data_store
                .append_removal_list(scratchpad.trim(), "scratchpad")?;
        }

        Ok(())
    }

    /// Processes pod data by managing scratchpad allocation and data distribution.
    ///
    /// This function handles the complex task of distributing pod graph data across multiple
    /// scratchpads, ensuring that each scratchpad stays within the 4MB size limit. It also
    /// manages the creation of additional scratchpads when needed and properly sorts the
    /// data to ensure pod_index and pod_ref entries are prioritized.
    ///
    /// # Parameters
    ///
    /// * `pod_address` - The hexadecimal address of the pod
    /// * `graph_data` - The TriG-formatted graph data as a byte vector
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if scratchpad operations fail.
    async fn process_pod_data(
        &mut self,
        pod_address: &str,
        graph_data: Vec<u8>,
    ) -> Result<(), Error> {
        const SCRATCHPAD_SIZE_LIMIT: usize = 4 * 1024 * 1024; // 4MB in bytes

        // Convert graph data to string for processing
        let graph_string: String = graph_data.into_iter().map(|b| b as char).collect();

        // Check current scratchpads for this pod
        let current_scratchpads = self
            .get_pod_scratchpads(pod_address)?
            .unwrap_or_else(Vec::new);

        // Calculate how many scratchpads we need for the data
        let data_size = graph_string.len();
        let required_scratchpads = data_size.div_ceil(SCRATCHPAD_SIZE_LIMIT);
        let required_scratchpads = std::cmp::max(1, required_scratchpads); // At least 1 scratchpad

        // Create additional scratchpads if needed
        let mut all_scratchpads = current_scratchpads.clone();
        while all_scratchpads.len() < required_scratchpads {
            let new_scratchpad = self.add_scratchpad(pod_address).await?;
            let new_address = new_scratchpad.to_hex();
            all_scratchpads.push(new_address.clone());

            // Add the new scratchpad to the graph with proper pod_index
            let scratchpad_iri = format!("ant://{new_address}");
            let pod_iri = format!("ant://{pod_address}");
            let index = (all_scratchpads.len() - 1).to_string();

            self.graph
                .put_quad(&scratchpad_iri, graph::HAS_INDEX, &index, Some(&pod_iri))?;

            // Add the modified date to the new scratchpad
            let date = Utc::now().to_rfc3339();
            self.graph.put_quad(
                &scratchpad_iri,
                graph::HAS_MODIFIED_DATE,
                &date,
                Some(&pod_iri),
            )?;

            // Update the key count
            let num_keys = self.key_store.get_num_keys();
            self.graph.update_key_count(pod_address, num_keys)?;
        }

        // Sort the graph data to prioritize pod_index and pod_ref entries
        let sorted_data = self.sort_graph_data(&graph_string);

        // Split the sorted data into chunks that fit in scratchpads
        let chunks = self.split_data_into_chunks(&sorted_data, SCRATCHPAD_SIZE_LIMIT);

        // Update scratchpads with the chunked data
        for (i, chunk) in chunks.iter().enumerate() {
            if i < all_scratchpads.len() {
                let scratchpad_address = &all_scratchpads[i];
                self.data_store
                    .update_scratchpad_data(scratchpad_address.trim(), chunk)?;
                self.data_store
                    .add_scratchpad_to_pod(pod_address, scratchpad_address)?;
            }
        }

        // Clear any unused scratchpads
        for scratchpad_address in all_scratchpads.iter().skip(chunks.len()) {
            self.data_store
                .remove_scratchpad_file(scratchpad_address.trim())?;
            self.data_store
                .append_removal_list(scratchpad_address.trim(), "scratchpad")?;
            self.graph
                .remove_scratchpad_entry(pod_address, scratchpad_address.trim())?;
            self.key_store
                .remove_scratchpad_key(scratchpad_address.trim())?;
        }

        // Add the pod pointer address to the update list
        self.data_store.append_update_list(pod_address)?;

        Ok(())
    }

    /// Sorts graph data to prioritize pod_index and pod_ref entries.
    ///
    /// This function ensures that statements containing pod_index predicates appear first,
    /// followed by statements containing pod_ref objects, with all other data following.
    /// This ordering is important for proper scratchpad linking and pod reference handling.
    ///
    /// The function properly handles multi-line TriG statements by grouping continuation
    /// lines (those starting with whitespace) with their subject line.
    ///
    /// # Parameters
    ///
    /// * `data` - The TriG-formatted graph data as a string
    ///
    /// # Returns
    ///
    /// Returns the sorted data as a string with prioritized entries first.
    pub fn sort_graph_data(&self, data: &str) -> String {
        let lines: Vec<&str> = data.lines().collect();
        let mut statements: Vec<Vec<&str>> = Vec::new();
        let mut current_statement: Vec<&str> = Vec::new();

        // Group lines into statements (subject + continuation lines)
        // FIXME: this works for the current library, but should probably make this more robust at some point to check
        // for proper TriG syntax, i.e. look for a ';' at the end of the previous line to know the following statment
        // is a continuation of the previous statement
        for line in lines {
            if line.trim().is_empty() {
                // Empty line - add to current statement if it exists, otherwise skip
                if !current_statement.is_empty() {
                    current_statement.push(line);
                }
            } else if line.starts_with(char::is_whitespace) {
                // Continuation line (starts with whitespace) - add to current statement
                if !current_statement.is_empty() {
                    current_statement.push(line);
                } else {
                    // Orphaned continuation line - treat as new statement
                    current_statement.push(line);
                }
            } else {
                // New subject line - save previous statement and start new one
                if !current_statement.is_empty() {
                    statements.push(current_statement);
                }
                current_statement = vec![line];
            }
        }

        // Don't forget the last statement
        if !current_statement.is_empty() {
            statements.push(current_statement);
        }

        // Sort statements based on the priority of their first (subject) line
        statements.sort_by(|a, b| {
            let a_priority = if !a.is_empty() {
                self.get_statement_priority(a)
            } else {
                2
            };
            let b_priority = if !b.is_empty() {
                self.get_statement_priority(b)
            } else {
                2
            };
            a_priority.cmp(&b_priority)
        });

        // Reconstruct the sorted data
        let mut result = Vec::new();
        for statement in statements {
            for line in statement {
                result.push(line);
            }
        }

        result.join("\n")
    }

    /// Determines the sorting priority for a TriG statement.
    ///
    /// # Parameters
    ///
    /// * `statement` - A vector of lines representing a complete TriG statement
    ///
    /// # Returns
    ///
    /// Returns a priority value where lower numbers indicate higher priority.
    fn get_statement_priority(&self, statement: &[&str]) -> u8 {
        // Check all lines in the statement for priority indicators
        for line in statement {
            if line.contains(graph::HAS_INDEX) {
                return 0; // Pod scratchpads should always be first in the scratchpad (pointer can only point to the first scratchpad)
            } else if line.contains(graph::POD_REF) {
                return 1; // Pod references are next for future enhancement to thread the data fetches
            }
        }
        2 // Everything else in the pod
    }

    /// Splits data into chunks that fit within the scratchpad size limit.
    ///
    /// This function intelligently splits the data while trying to preserve line boundaries
    /// when possible. It ensures that no chunk exceeds the specified size limit. Each chunk
    /// starts with a timestamp comment in the format '#<timestamp>\n' (37 bytes total).
    ///
    /// # Parameters
    ///
    /// * `data` - The data to split
    /// * `chunk_size` - Maximum size for each chunk in bytes (includes the 37-byte timestamp comment)
    ///
    /// # Returns
    ///
    /// Returns a vector of string chunks, each within the size limit and starting with a timestamp comment.
    pub fn split_data_into_chunks(&self, data: &str, chunk_size: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();

        // Reserve bytes for the timestamp comment: '#' + RFC3339 timestamp + '\n'
        // RFC3339 format can be up to 35 characters (e.g., "2024-01-15T10:30:45.123456789+00:00")
        // Plus '#' (1 char) and '\n' (1 char) = 37 bytes total
        const TIMESTAMP_COMMENT_SIZE: usize = 37;
        let effective_chunk_size = chunk_size.saturating_sub(TIMESTAMP_COMMENT_SIZE);

        // Handle the case where data doesn't end with newline
        let data_with_newline = if data.ends_with('\n') {
            data.to_string()
        } else {
            format!("{data}\n")
        };

        for line in data_with_newline.lines() {
            let line_with_newline = format!("{line}\n");

            // If adding this line would exceed the effective chunk size, start a new chunk
            if !current_chunk.is_empty()
                && current_chunk.len() + line_with_newline.len() > effective_chunk_size
            {
                chunks.push(current_chunk.clone());
                current_chunk.clear();
            }

            // If a single line is larger than effective_chunk_size, we need to split it
            if line_with_newline.len() > effective_chunk_size {
                // Add any existing chunk first
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.clone());
                    current_chunk.clear();
                }

                // Split the large line into smaller pieces (without adding extra newlines)
                let line_str = line; // Use the line without the newline we added
                let line_bytes = line_str.as_bytes();
                for chunk_start in (0..line_bytes.len()).step_by(effective_chunk_size) {
                    let chunk_end =
                        std::cmp::min(chunk_start + effective_chunk_size, line_bytes.len());
                    let chunk_bytes = &line_bytes[chunk_start..chunk_end];
                    if let Ok(chunk_str) = std::str::from_utf8(chunk_bytes) {
                        // Only add newline to the last chunk of this line
                        if chunk_end == line_bytes.len() {
                            chunks.push(format!("{chunk_str}\n"));
                        } else {
                            chunks.push(chunk_str.to_string());
                        }
                    }
                }
            } else {
                current_chunk.push_str(&line_with_newline);
            }
        }

        // Add the final chunk if it has content
        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        // Ensure we always have at least one chunk (even if empty)
        if chunks.is_empty() {
            chunks.push(String::new());
        }

        // Add timestamp comment to the beginning of each chunk
        let timestamp = chrono::Utc::now().to_rfc3339();
        let timestamp_comment = format!("#{timestamp}\n");

        for chunk in chunks.iter_mut() {
            *chunk = format!("{timestamp_comment}{chunk}");
        }

        chunks
    }

    /// Selects the newest scratchpad from a vector of scratchpads based on timestamp comments.
    ///
    /// This function reads the encrypted data from each scratchpad, looks for a timestamp comment
    /// in the first line (format: #<RFC3339_timestamp>), and returns the scratchpad with the
    /// latest timestamp. If only one scratchpad has a timestamp, it's assumed to be the newest.
    /// If none have timestamps, the first scratchpad in the vector is returned.
    ///
    /// # Parameters
    ///
    /// * `scratchpads` - Vector of scratchpads to compare
    ///
    /// # Returns
    ///
    /// Returns the scratchpad with the latest timestamp, or the first one if no timestamps are found.
    fn select_newest_scratchpad(scratchpads: Vec<Scratchpad>) -> Scratchpad {
        if scratchpads.is_empty() {
            panic!("Cannot select from empty scratchpads vector");
        }

        if scratchpads.len() == 1 {
            return scratchpads[0].clone();
        }

        let mut newest_scratchpad = &scratchpads[0];
        let mut newest_timestamp: Option<chrono::DateTime<chrono::Utc>> = None;

        for scratchpad in &scratchpads {
            // Extract the encrypted data and convert to string
            let data = scratchpad.encrypted_data();
            if let Ok(data_string) = String::from_utf8(data.to_vec()) {
                // Check if the first line is a timestamp comment
                if let Some(first_line) = data_string.lines().next() {
                    if first_line.starts_with('#') && first_line.len() > 1 {
                        let timestamp_str = &first_line[1..]; // Remove the '#' prefix

                        // Try to parse the timestamp
                        if let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(timestamp_str) {
                            let utc_timestamp = timestamp.with_timezone(&chrono::Utc);

                            // Check if this is the newest timestamp so far
                            if newest_timestamp.is_none()
                                || utc_timestamp > newest_timestamp.unwrap()
                            {
                                newest_timestamp = Some(utc_timestamp);
                                newest_scratchpad = scratchpad;
                            }
                        }
                    }
                }
            }
        }

        // If we found at least one timestamp, return the newest one
        // If no timestamps were found, return the first scratchpad
        newest_scratchpad.clone()
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
    pub async fn add_pod(&mut self, pod_name: &str) -> Result<(String, String), Error> {
        let pod_address = self.add_pointer().await?;
        let pod_address = pod_address.to_hex();
        let pod_address = pod_address.as_str();
        let scratchpad_address = self.add_scratchpad(pod_address).await?;
        let scratchpad_address = scratchpad_address.to_hex();
        let scratchpad_address = scratchpad_address.as_str();

        // Add the scratchpad address to the pointer files
        self.data_store
            .update_pointer_target(pod_address, scratchpad_address)?;

        let configuration_address = self.key_store.get_configuration_address()?;
        let configuration_address = configuration_address.as_str();
        let configuration_scratchpad_address =
            self.key_store.get_configuration_scratchpad_address()?;
        let configuration_scratchpad_address = configuration_scratchpad_address.as_str();
        self.data_store
            .update_pointer_target(configuration_address, configuration_scratchpad_address)?;

        // Get the number of keys to store in the graph
        let num_keys = self.key_store.get_num_keys();

        // Add the pointer address to the graph
        let (graph, configuration) = self.graph.add_pod_entry(
            pod_name,
            pod_address,
            scratchpad_address,
            configuration_address,
            configuration_scratchpad_address,
            num_keys,
        )?;

        // Process the pod data with proper scratchpad management
        self.process_pod_data(pod_address, graph).await?;

        // Update the configuration graph with the updated key count
        let num_keys = self.key_store.get_num_keys();
        self.graph
            .update_key_count(configuration_address, num_keys)?;

        self.process_pod_data(configuration_address, configuration)
            .await?;

        Ok((pod_address.to_string(), scratchpad_address.to_string()))
    }

    /// Removes a pod and all its associated data from the local store and network.
    ///
    /// This function completely removes a pod from the Colony system, including:
    /// - Removing the pod entry from the graph database
    /// - Removing all associated scratchpad data
    /// - Removing the pod from the configuration pod's reference list
    /// - Cleaning up local files and key store entries
    /// - Adding addresses to the removal queue for network cleanup
    ///
    /// The pod will be marked for removal from the Autonomi network on the next call to `upload_all()`.
    /// This operation cannot be undone once the changes are uploaded to the network.
    ///
    /// # Parameters
    ///
    /// * `pod_address` - The hexadecimal address of the pod to remove
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the pod was successfully removed from local storage and queued for network removal.
    ///
    /// Returns an `Error` if:
    /// - The pod address does not exist in the local store
    /// - The pod address is the configuration pod (cannot be removed)
    /// - Graph database operations fail
    /// - Local file operations fail
    /// - Key store operations fail
    ///
    /// # Safety
    ///
    /// - **Cannot remove configuration pod**: The configuration pod is protected and cannot be removed
    /// - **Irreversible operation**: Once uploaded to the network, the pod removal cannot be undone
    /// - **Cascading removal**: All scratchpads associated with the pod are also removed
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // Create a pod to demonstrate removal
    /// let (pod_address, _) = pod_manager.add_pod("Temporary Pod").await?;
    ///
    /// // Add some data to the pod
    /// let subject_data = r#"{
    ///     "@context": {"schema": "http://schema.org/"},
    ///     "@type": "schema:Document",
    ///     "schema:name": "Temporary Document"
    /// }"#;
    /// pod_manager.put_subject_data(&pod_address, &pod_address, subject_data).await?;
    ///
    /// // Remove the pod (this only removes it locally and queues for network removal)
    /// pod_manager.remove_pod(&pod_address).await?;
    ///
    /// // The pod is now removed from local storage but still exists on the network
    /// // Upload the removal to the network
    /// pod_manager.upload_all().await?;
    ///
    /// // Now the pod is completely removed from both local storage and the network
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`add_pod`] - Create a new pod
    /// - [`rename_pod`] - Rename an existing pod
    /// - [`upload_all`] - Upload pending removals to the network
    /// - [`list_my_pods`] - List all local pods
    pub async fn remove_pod(&mut self, pod_address: &str) -> Result<(), Error> {
        let pod_address = self.graph.check_pod_exists(pod_address)?;
        let pod_address = pod_address.trim();
        let configuration_address = self.key_store.get_configuration_address()?;
        let configuration_address = configuration_address.as_str();

        // Return an error if trying to remove the configuration pod
        if pod_address == configuration_address {
            return Err(Error::Pod("Cannot remove configuration pod".to_string()));
        }

        // Check current scratchpads for this pod
        let pod_scratchpads = self
            .get_pod_scratchpads(pod_address)?
            .unwrap_or_else(Vec::new);

        // Remove the pod from the graph
        let configuration = self.graph.remove_pod_entry(
            pod_address,
            pod_scratchpads.clone(),
            configuration_address,
        )?;
        self.process_pod_data(configuration_address, configuration)
            .await?;

        // Process the pod data with proper scratchpad management
        self.remove_pod_data(pod_address, pod_scratchpads).await?;

        Ok(())
    }

    /// Renames an existing pod in the local store and queues the change for network upload.
    ///
    /// This function updates the human-readable name of a pod in the graph database.
    /// The pod's address remains unchanged, but its display name is updated throughout
    /// the system. The change will be uploaded to the Autonomi network on the next call
    /// to `upload_all()`.
    ///
    /// # Parameters
    ///
    /// * `pod_address` - The hexadecimal address of the pod to rename
    /// * `new_name` - The new human-readable name for the pod
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the pod was successfully renamed in local storage and queued for network upload.
    ///
    /// Returns an `Error` if:
    /// - The pod address does not exist in the local store
    /// - Graph database operations fail
    /// - Local file operations fail
    /// - The new name is invalid or empty
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // Create a pod with an initial name
    /// let (pod_address, _) = pod_manager.add_pod("Initial Name").await?;
    ///
    /// // Add some data to the pod
    /// let subject_data = r#"{
    ///     "@context": {"schema": "http://schema.org/"},
    ///     "@type": "schema:Collection",
    ///     "schema:name": "My Collection",
    ///     "schema:description": "A collection of important items"
    /// }"#;
    /// pod_manager.put_subject_data(&pod_address, &pod_address, subject_data).await?;
    ///
    /// // Rename the pod to something more descriptive
    /// pod_manager.rename_pod(&pod_address, "Important Documents Collection").await?;
    ///
    /// // The pod is now renamed locally, upload the change to the network
    /// pod_manager.upload_all().await?;
    ///
    /// // Verify the new name appears in the pod list
    /// let pods = pod_manager.list_my_pods().await?;
    /// let renamed_pod = pods.iter().find(|p| p.address == pod_address).unwrap();
    /// assert_eq!(renamed_pod.name, "Important Documents Collection");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`add_pod`] - Create a new pod with a name
    /// - [`remove_pod`] - Remove an existing pod
    /// - [`list_my_pods`] - List all pods with their current names
    /// - [`upload_all`] - Upload the name change to the network
    pub async fn rename_pod(&mut self, pod_address: &str, new_name: &str) -> Result<(), Error> {
        let pod_address = self.graph.check_pod_exists(pod_address)?;
        let pod_address = pod_address.trim();

        // Rename the pod in the graph
        let graph = self.graph.rename_pod_entry(pod_address, new_name)?;

        // Process the pod data with proper scratchpad management
        self.process_pod_data(pod_address, graph).await?;

        Ok(())
    }

    /// Adds a reference from one pod to another pod in the graph database.
    ///
    /// This function creates a semantic link between two pods, allowing for the creation
    /// of pod networks and hierarchies. The reference is stored in the graph database
    /// and will be included when the referencing pod is uploaded to the network.
    /// Referenced pods can be discovered and downloaded automatically using `refresh_ref()`.
    /// Use the associated `remove_pod_ref()` to remove a reference from a local pod.
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
    /// pod_manager.add_pod_ref(&main_pod, &sub_pod).await?;
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
    /// - [`remvoe_pod_ref`] - Remove a pod reference in a local pod
    /// - [`refresh_ref`] - Download referenced pods from the network
    /// - [`upload_all`] - Upload pod references to the network
    pub async fn add_pod_ref(
        &mut self,
        pod_address: &str,
        pod_ref_address: &str,
    ) -> Result<(), Error> {
        let pod_address = self.graph.check_pod_exists(pod_address)?;
        let pod_address = pod_address.trim();
        let configuration_address = self.key_store.get_configuration_address()?;
        let configuration_address = configuration_address.as_str();

        // Check if the pod reference address is to a local pod
        let is_local = self.data_store.address_is_pointer(pod_ref_address)?;

        // Add the pointer address to the graph
        let (graph, configuration) = self.graph.pod_ref_entry(
            pod_address,
            pod_ref_address,
            configuration_address,
            true,
            is_local,
        )?;

        // Process the pod data with proper scratchpad management
        self.process_pod_data(pod_address, graph).await?;
        // Update the configuration graph with the updated key count
        let num_keys = self.key_store.get_num_keys();
        self.graph
            .update_key_count(configuration_address, num_keys)?;
        self.process_pod_data(configuration_address, configuration)
            .await?;

        Ok(())
    }

    /// Removes a reference to a pod in a local pod in the graph database.
    ///
    /// This function removes a semantic link between two pods. It is the opposite of `add_pod_ref()`
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
    /// // Remove a reference from main pod to sub pod
    /// pod_manager.remove_pod_ref(&main_pod, &sub_pod).await?;
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
    /// - [`add_pod_ref`] - Create a pod reference in a local pod
    /// - [`refresh_ref`] - Download referenced pods from the network
    /// - [`upload_all`] - Upload pod references to the network
    pub async fn remove_pod_ref(
        &mut self,
        pod_address: &str,
        pod_ref_address: &str,
    ) -> Result<(), Error> {
        let pod_address = self.graph.check_pod_exists(pod_address)?;
        let pod_address = pod_address.trim();
        let configuration_address = self.key_store.get_configuration_address()?;
        let configuration_address = configuration_address.as_str();

        // Check if the pod reference address is to a local pod
        let is_local = self.data_store.address_is_pointer(pod_ref_address)?;

        // Remove the pointer address to the graph
        let (graph, configuration) = self.graph.pod_ref_entry(
            pod_address,
            pod_ref_address,
            configuration_address,
            false,
            is_local,
        )?;

        // Process the pod data with proper scratchpad management
        self.process_pod_data(pod_address, graph).await?;
        // Update the configuration graph with the updated key count
        let num_keys = self.key_store.get_num_keys();
        self.graph
            .update_key_count(configuration_address, num_keys)?;
        self.process_pod_data(configuration_address, configuration)
            .await?;

        Ok(())
    }

    async fn add_scratchpad(&mut self, pod_address: &str) -> Result<ScratchpadAddress, Error> {
        // Derive a new key for the pod scratchpad
        let scratchpad_key: SecretKey = self.create_scratchpad_key().await?;
        let scratchpad_address: ScratchpadAddress =
            ScratchpadAddress::new(scratchpad_key.clone().public_key());

        // Create a new file in the pod directory from the address
        self.data_store
            .create_scratchpad_file(scratchpad_address.clone().to_hex().as_str())?;
        self.data_store
            .add_scratchpad_to_pod(pod_address, &scratchpad_address.to_hex())?;

        Ok(scratchpad_address)
    }

    async fn add_pointer(&mut self) -> Result<PointerAddress, Error> {
        // Derive a new key for the pod scratchpad
        let pointer_key: SecretKey = self.create_pointer_key().await?;
        let pointer_address = PointerAddress::new(pointer_key.clone().public_key());

        // Create a new file in the pod directory from the address
        self.data_store
            .create_pointer_file(pointer_address.clone().to_hex().as_str())?;
        self.data_store
            .append_update_list(pointer_address.clone().to_hex().as_str())?;

        Ok(pointer_address)
    }

    /// Lists all pods owned by the user.
    ///
    /// This function retrieves a comprehensive list of all pods that belong to the current user,
    /// including both locally created pods and pods downloaded from the network. The results
    /// include pod metadata such as names, addresses, creation information, and reference counts.
    ///
    /// # Returns
    ///
    /// Returns a JSON object containing SPARQL query results with the following structure:
    /// - `results.bindings` - Array of pod objects, each containing:
    ///   - `pod.value` - The pod's Autonomi address
    ///   - `name.value` - The human-readable pod name
    ///   - `created.value` - ISO 8601 timestamp of pod creation
    ///   - Additional metadata fields as available
    ///
    /// Returns an `Error` if:
    /// - The graph database query fails
    /// - JSON parsing fails
    /// - Local storage access fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // Get all user pods
    /// let pods_result = pod_manager.list_my_pods()?;
    ///
    /// // Parse the results
    /// if let Some(bindings) = pods_result["results"]["bindings"].as_array() {
    ///     println!("Found {} pods:", bindings.len());
    ///
    ///     for pod in bindings {
    ///         let pod_address = pod["pod"]["value"].as_str().unwrap_or("unknown");
    ///         let pod_name = pod["name"]["value"].as_str().unwrap_or("unnamed");
    ///         let created = pod["created"]["value"].as_str().unwrap_or("unknown");
    ///
    ///         println!("Pod: {} ({})", pod_name, pod_address);
    ///         println!("Created: {}", created);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`add_pod`] - Create new pods
    /// - [`list_pod_subjects`] - List subjects within a specific pod
    /// - [`search`] - Search across all pods
    /// - [`refresh_cache`] - Update the list with network changes
    pub fn list_my_pods(&self) -> Result<Value, Error> {
        let configuration_address = self.key_store.get_configuration_address()?;
        let search_results = self.graph.get_my_pods(configuration_address.as_str())?;

        // Parse the SPARQL JSON results and return them
        let results: Value = serde_json::from_str(&search_results)?;

        Ok(results)
    }

    /// Lists all subjects (resources) contained within a specific pod.
    ///
    /// This function retrieves all subject addresses that have metadata stored in the specified pod.
    /// Subjects typically represent files, documents, or other resources that have been catalogued
    /// with semantic metadata. The function returns the Autonomi addresses of these subjects,
    /// which can then be used to retrieve detailed metadata or the actual files.
    ///
    /// # Parameters
    ///
    /// * `pod_address` - The hexadecimal Autonomi address of the pod to query
    ///
    /// # Returns
    ///
    /// Returns a vector of subject addresses (as hex strings) found in the pod, or an `Error` if:
    /// - The pod address is invalid or doesn't exist locally
    /// - The graph database query fails
    /// - Local storage access fails
    ///
    /// The returned addresses can be used with [`get_subject_data`] to retrieve full metadata
    /// for each subject.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// let pod_address = "80e79010a13e7eee779f799d99a20b418436828269b18192d92940bc9ddbfe295a7e1823d7bff75c59cbacbdea101a0d";
    ///
    /// // Get all subjects in the pod
    /// let subjects = pod_manager.list_pod_subjects(pod_address)?;
    ///
    /// println!("Found {} subjects in pod:", subjects.len());
    /// for subject_address in subjects {
    ///     println!("Subject: {}", subject_address);
    ///
    ///     // Get detailed metadata for each subject
    ///     let metadata = pod_manager.get_subject_data(&subject_address).await?;
    ///     let metadata_json: serde_json::Value = serde_json::from_str(&metadata)?;
    ///
    ///     // Extract subject name if available
    ///     if let Some(bindings) = metadata_json["results"]["bindings"].as_array() {
    ///         for binding in bindings {
    ///             if let Some(name) = binding["object"]["value"].as_str() {
    ///                 if binding["predicate"]["value"].as_str() == Some("http://schema.org/name") {
    ///                     println!("  Name: {}", name);
    ///                 }
    ///             }
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`list_my_pods`] - List all user pods
    /// - [`get_subject_data`] - Get detailed metadata for a subject
    /// - [`put_subject_data`] - Add metadata for subjects to pods
    /// - [`search`] - Search for subjects across all pods
    pub fn list_pod_subjects(&self, pod_address: &str) -> Result<Vec<String>, Error> {
        let pod_address = self.graph.check_pod_exists(pod_address)?;
        let pod_address = pod_address.trim();
        // Get all subjects in the pod from the graph database
        let subjects = self.graph.get_pod_subjects(pod_address)?;
        Ok(subjects)
    }

    ///////////////////////////////////////////
    // Autonomi network operations
    ///////////////////////////////////////////

    // Not used today, ignoring the unused warning
    #[allow(dead_code)]
    async fn get_address_type(&mut self, address: &str) -> Result<(Analysis, bool), Error> {
        // get the type stored on the network
        let mut create_mode = false;
        let pod_type = self
            .client
            .analyze_address(address, false)
            .await
            .unwrap_or_else(|e| -> Analysis {
                match e {
                    AnalysisError::FailedGet => {
                        info!("Address currently does not hold data: {}", address);
                        create_mode = true;
                        // check if address is a directory (pointer) or a file (scratchpad)
                        // and return a dummy analysis type for processing, else
                        // return a chunk to indicate an error
                        if self.data_store.address_is_pointer(address).unwrap_or(false) {
                            Analysis::Pointer(Pointer::new(
                                &SecretKey::random(),
                                0,
                                PointerTarget::ScratchpadAddress(ScratchpadAddress::new(
                                    SecretKey::random().public_key(),
                                )),
                            ))
                        } else if self
                            .data_store
                            .address_is_scratchpad(address)
                            .unwrap_or(false)
                        {
                            Analysis::Scratchpad(Scratchpad::new(
                                &SecretKey::random(),
                                0,
                                &Bytes::new(),
                                0,
                            ))
                        } else {
                            warn!("Address is neither a pointer nor a scratchpad: {}", address);
                            Analysis::Chunk(Chunk::new(Bytes::new()))
                        }
                    }
                    _ => {
                        warn!("Address error: {}", e);
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
        let update_list = self.data_store.get_update_list()?;
        info!(
            "Starting upload_all with {} pods to upload",
            update_list.pods.len()
        );

        // Phase 1: Preprocess all operations and collect data
        let mut removal_operations = Vec::new();
        let mut upload_operations = Vec::new();

        // Preprocess removals
        info!(
            "Preprocessing {} pointer removals and {} scratchpad removals",
            update_list.remove.pointers.len(),
            update_list.remove.scratchpads.len()
        );

        for pointer_address in &update_list.remove.pointers {
            removal_operations.push((
                "pointer".to_string(),
                pointer_address.clone(),
                pointer_address.clone(),
            ));
        }

        for scratchpad_address in &update_list.remove.scratchpads {
            removal_operations.push((
                "scratchpad".to_string(),
                scratchpad_address.clone(),
                "".to_string(),
            ));
        }

        // Preprocess uploads
        info!("Preprocessing {} pod uploads", update_list.pods.len());
        for pod_address in update_list.pods.keys() {
            let address = self.graph.check_pod_exists(pod_address)?;
            let address = address.trim();

            // Get the target scratchpad address
            let target = self.data_store.get_pointer_target(address)?;
            let target = target.trim();

            // Add pointer upload operation
            upload_operations.push((
                "pointer".to_string(),
                address.to_string(),
                target.to_string(),
            ));

            // Get all scratchpads for this pod
            let data = self.data_store.get_scratchpad_data(target)?;
            let scratchpads = self.graph.get_pod_scratchpads_from_string(data.trim())?;

            // Add scratchpad upload operations
            for scratchpad_address in scratchpads {
                let scratchpad_address = scratchpad_address.trim();
                let scratchpad_data = self.data_store.get_scratchpad_data(scratchpad_address)?;
                upload_operations.push((
                    "scratchpad".to_string(),
                    scratchpad_address.to_string(),
                    scratchpad_data.trim().to_string(),
                ));
            }
        }

        // Phase 2: Execute all operations with maximum concurrency - removals and uploads simultaneously
        info!(
            "Executing {} removal operations and {} upload operations concurrently",
            removal_operations.len(),
            upload_operations.len()
        );

        self.execute_all_operations_concurrent(removal_operations, upload_operations)
            .await?;

        // Clear out the update list
        self.data_store.clear_update_list()?;
        Ok(())
    }

    /// Uploads a specific pod to the Autonomi network.
    ///
    /// This function uploads a single pod and all its associated scratchpads to the Autonomi network.
    /// It handles both creating new network objects and updating existing ones based on their current
    /// state. The function automatically determines whether each address needs to be created or updated
    /// by checking the network state.
    ///
    /// # Parameters
    ///
    /// * `address` - The hexadecimal Autonomi address of the pod to upload
    ///
    /// # Process
    ///
    /// 1. Validates that the pod exists locally
    /// 2. Attempts to update the pod's pointer on the network
    /// 3. If the pointer doesn't exist, creates a new pointer
    /// 4. Retrieves all scratchpads associated with the pod
    /// 5. For each scratchpad, attempts to update or create as needed
    /// 6. Removes the pod address from the upload queue upon success
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful upload, or an `Error` if:
    /// - The pod address is invalid or doesn't exist locally
    /// - Network communication fails
    /// - Payment processing fails
    /// - Local file operations fail
    /// - Address analysis fails
    ///
    /// # Network Costs
    ///
    /// This operation incurs network costs for:
    /// - Creating new pointers (if the pod is new)
    /// - Creating new scratchpads (if additional storage is needed)
    /// - Updates to existing pointers and scratchpads are free
    ///
    /// Costs are automatically paid using the configured wallet.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // Create a new pod
    /// let (pod_address, _) = pod_manager.add_pod("My Documents").await?;
    ///
    /// // Add some metadata to the pod
    /// let subject_address = "c859818c623ce4fc0899c2ab43061b19caa0b0598eec35ef309dbe50c8af8d59";
    /// let metadata = r#"{
    ///     "@context": "http://schema.org/",
    ///     "@type": "Dataset",
    ///     "name": "Research Data",
    ///     "description": "Important research findings"
    /// }"#;
    /// pod_manager.put_subject_data(&pod_address, subject_address, metadata).await?;
    ///
    /// // Upload the specific pod to the network
    /// pod_manager.upload_pod(&pod_address).await?;
    ///
    /// println!("Pod {} uploaded successfully!", pod_address);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`upload_all`] - Upload all pending pods at once
    /// - [`add_pod`] - Create pods that need uploading
    /// - [`put_subject_data`] - Modify pods that need uploading
    /// - [`refresh_cache`] - Download updates from the network
    pub async fn upload_pod(&mut self, address: &str) -> Result<(), Error> {
        let mut create_mode = false;
        let address = self.graph.check_pod_exists(address)?;
        let address = address.trim();

        // check if there is a pointer stored at this address on the network by trying to download it
        let target = self.data_store.get_pointer_target(address)?;
        let target = target.trim();
        match self.update_pointer(address, target).await {
            Ok(_) => {}
            Err(e) => {
                match e {
                    Error::Pointer(ref boxed_err)
                        if matches!(**boxed_err, PointerError::CannotUpdateNewPointer) =>
                    {
                        info!(
                            "Pointer not found on network, creating new pointer: {}",
                            address
                        );
                        create_mode = true;
                    }
                    // Catch Pointer(Network(GetRecordError(RecordNotFound))) error when there is nothing on the network
                    Error::Pointer(ref boxed_err)
                        if matches!(
                            **boxed_err,
                            PointerError::GetError(GetError::RecordNotFound)
                        ) =>
                    {
                        info!(
                            "Pointer not found on network, creating new pointer: {}",
                            address
                        );
                        create_mode = true;
                    }
                    _ => {
                        error!("Error occurred: {:?}", e); // Log the error
                        return Err(e); // Propagate the error to the higher-level function
                    }
                }
            }
        }

        // If the pointer didn't exist, call create_pointer()
        if create_mode {
            self.create_pointer(address, target).await?;
        }

        create_mode = false;

        // Get all of the scratchpads for the pod
        let data = self.data_store.get_scratchpad_data(target)?;
        let scratchpads = self.graph.get_pod_scratchpads_from_string(data.trim())?;

        // Loop through each scratchpad address
        for scratchpad_address in scratchpads {
            let address = scratchpad_address.trim();
            let data = self.data_store.get_scratchpad_data(address)?;
            let data = data.trim();

            match self.update_scratchpad(address, data).await {
                Ok(_) => {}
                Err(e) => {
                    match e {
                        Error::Scratchpad(ref boxed_err)
                            if matches!(
                                **boxed_err,
                                ScratchpadError::CannotUpdateNewScratchpad
                            ) =>
                        {
                            info!(
                                "Scratchpad not found on network, creating new scratchpad: {}",
                                address
                            );
                            create_mode = true;
                        }
                        // Catch Scratchpad(Network(GetRecordError(RecordNotFound))) error when there is nothing on the network
                        Error::Scratchpad(ref boxed_err)
                            if matches!(
                                **boxed_err,
                                ScratchpadError::GetError(GetError::RecordNotFound)
                            ) =>
                        {
                            info!(
                                "Scratchpad not found on network, creating new scratchpad: {}",
                                address
                            );
                            create_mode = true;
                        }
                        _ => {
                            error!("Error occurred: {:?}", e); // Log the error
                            return Err(e); // Propagate the error to the higher-level function
                        }
                    }
                }
            }

            // If the pointer didn't exist, call create_pointer()
            if create_mode {
                self.create_scratchpad(address, data).await?;
            }
        }

        debug!("Pod {} uploaded successfully", address);
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
        let (pointer_cost, _pointer_address) =
            self.client.pointer_put(pointer, payment_option).await?;
        debug!("Pointer upload cost: {pointer_cost:?}");

        Ok(pointer_cost.to_string())
    }

    async fn create_scratchpad(&mut self, address: &str, data: &str) -> Result<String, Error> {
        let key_string = self.key_store.get_scratchpad_key(address.to_string())?;
        let key: SecretKey = SecretKey::from_hex(key_string.trim())?;

        // Create new publicly readable scratchpad
        let scratchpad_address: ScratchpadAddress =
            ScratchpadAddress::new(key.clone().public_key());
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
        let (scratchpad_cost, _scratchpad_address) = self
            .client
            .scratchpad_put(scratchpad, payment_option.clone())
            .await?;
        debug!("Scratchpad cost: {scratchpad_cost:?}");

        Ok(scratchpad_cost.to_string())
    }

    async fn update_pointer(&mut self, address: &str, target: &str) -> Result<(), Error> {
        let key_string = self.key_store.get_pointer_key(address.to_string())?;
        let key: SecretKey = SecretKey::from_hex(key_string.trim())?;

        let pointer_address = PointerAddress::from_hex(address)?;
        let pointer = self.client.pointer_get(&pointer_address).await?;

        // Create the target address
        let target_address = ScratchpadAddress::from_hex(target)?;
        let target = PointerTarget::ScratchpadAddress(target_address);

        // Update the pointer counter and target
        self.client.pointer_update(&key, target).await?;
        debug!("Pointer updated");

        // Update the local pointer file counter
        let pointer_count = pointer.counter() + 1;
        self.data_store
            .update_pointer_count(address, pointer_count)?;
        Ok(())
    }

    async fn update_scratchpad(&mut self, address: &str, data: &str) -> Result<(), Error> {
        let key_string = self.key_store.get_scratchpad_key(address.to_string())?;
        let key: SecretKey = SecretKey::from_hex(key_string.trim())?;

        // get the scratchpad to make sure it exists and to get the current counter value
        let scratchpad_address = ScratchpadAddress::from_hex(address)?; // Lookup the key for the pod pointer from the key store
        let scratchpad = match self.client.scratchpad_get(&scratchpad_address).await {
            Ok(scratchpad) => scratchpad,
            Err(e) => match e {
                ScratchpadError::Fork(scratchpads) => Self::select_newest_scratchpad(scratchpads),
                _ => return Err(Error::Scratchpad(Box::new(e))),
            },
        };

        // Update the scratchpad contents and its counter
        let scratchpad = Scratchpad::new_with_signature(
            key.clone().public_key(),
            0,
            Bytes::from(data.to_owned()),
            scratchpad.counter() + 1,
            key.sign(Scratchpad::bytes_for_signature(
                scratchpad_address,
                0,
                &Bytes::from(data.to_owned()),
                scratchpad.counter() + 1,
            )),
        );

        // Put the new scratchpad on the network
        let payment_option = PaymentOption::from(self.wallet);
        let (scratchpad_cost, _scratchpad_address) = self
            .client
            .scratchpad_put(scratchpad, payment_option.clone())
            .await?;
        info!("Scratchpad update cost: {scratchpad_cost:?}");
        debug!("Scratchpad updated");

        Ok(())
    }

    /// Execute all operations with maximum concurrency - all client operations run simultaneously
    async fn execute_all_operations_concurrent(
        &mut self,
        removal_operations: Vec<(String, String, String)>,
        upload_operations: Vec<(String, String, String)>,
    ) -> Result<(), Error> {
        // Phase 1: Collect all keys and prepare data structures upfront
        let mut removal_data = Vec::new();
        let mut upload_pointer_data = Vec::new();
        let mut upload_scratchpad_data = Vec::new();

        // Create payment option upfront
        let payment_option = PaymentOption::from(self.wallet);

        // Collect removal operation data
        for (op_type, address, data) in removal_operations {
            match op_type.as_str() {
                "pointer" => {
                    if let Ok(key_string) = self.key_store.get_free_pointer_key(address.clone()) {
                        if let Ok(key) = SecretKey::from_hex(key_string.trim()) {
                            removal_data.push(("pointer".to_string(), address, data, key));
                        }
                    }
                }
                "scratchpad" => {
                    if let Ok(key_string) = self.key_store.get_scratchpad_key(address.clone()) {
                        if let Ok(key) = SecretKey::from_hex(key_string.trim()) {
                            removal_data.push(("scratchpad".to_string(), address, data, key));
                        }
                    }
                }
                _ => continue,
            }
        }

        // Collect upload operation data
        for (op_type, address, data_or_target) in upload_operations {
            match op_type.as_str() {
                "pointer" => {
                    if let Ok(key_string) = self.key_store.get_pointer_key(address.clone()) {
                        if let Ok(key) = SecretKey::from_hex(key_string.trim()) {
                            upload_pointer_data.push((address, data_or_target, key));
                        }
                    }
                }
                "scratchpad" => {
                    if let Ok(key_string) = self.key_store.get_scratchpad_key(address.clone()) {
                        if let Ok(key) = SecretKey::from_hex(key_string.trim()) {
                            upload_scratchpad_data.push((address, data_or_target, key));
                        }
                    }
                }
                _ => continue,
            }
        }

        // Phase 2: Execute ALL client operations concurrently
        use futures::future::BoxFuture;
        let mut all_futures: Vec<BoxFuture<'_, Result<(), Error>>> = Vec::new();

        // Add removal futures - all concurrent
        for (op_type, address, data, key) in removal_data {
            let client = &self.client;
            let addr_clone = address.clone();
            let data_clone = data.clone();
            let payment_opt = payment_option.clone();

            if op_type == "pointer" {
                let future = Box::pin(async move {
                    let pointer_address = PointerAddress::from_hex(&addr_clone)?;
                    match client.pointer_get(&pointer_address).await {
                        Ok(_) => {
                            let target_address = ScratchpadAddress::from_hex(&data_clone)?;
                            let target = PointerTarget::ScratchpadAddress(target_address);
                            client.pointer_update(&key, target).await?;
                            info!("Successfully removed pointer: {}", addr_clone);
                        }
                        Err(_) => {
                            info!(
                                "Pointer {} not found on network, already removed",
                                addr_clone
                            );
                        }
                    }
                    Ok::<(), Error>(())
                });
                all_futures.push(future);
            } else if op_type == "scratchpad" {
                let future = Box::pin(async move {
                    let scratchpad_address = ScratchpadAddress::from_hex(&addr_clone)?;
                    match client.scratchpad_get(&scratchpad_address).await {
                        Ok(scratchpad) => {
                            // Create updated scratchpad with empty data (removal)
                            let bytes = Bytes::from("".as_bytes().to_vec());
                            let updated_scratchpad = Scratchpad::new_with_signature(
                                key.clone().public_key(),
                                0,
                                bytes.clone(),
                                scratchpad.counter() + 1,
                                key.sign(Scratchpad::bytes_for_signature(
                                    scratchpad_address,
                                    0,
                                    &bytes,
                                    scratchpad.counter() + 1,
                                )),
                            );
                            client
                                .scratchpad_put(updated_scratchpad, payment_opt)
                                .await?;
                            info!("Successfully removed scratchpad: {}", addr_clone);
                        }
                        Err(e) => {
                            match e {
                                ScratchpadError::Fork(scratchpads) => {
                                    let scratchpad = Self::select_newest_scratchpad(scratchpads);
                                    // Create updated scratchpad with empty data (removal)
                                    let bytes = Bytes::from("".as_bytes().to_vec());
                                    let updated_scratchpad = Scratchpad::new_with_signature(
                                        key.clone().public_key(),
                                        0,
                                        bytes.clone(),
                                        scratchpad.counter() + 1,
                                        key.sign(Scratchpad::bytes_for_signature(
                                            scratchpad_address,
                                            0,
                                            &bytes,
                                            scratchpad.counter() + 1,
                                        )),
                                    );
                                    client
                                        .scratchpad_put(updated_scratchpad, payment_opt)
                                        .await?;
                                    info!("Successfully removed scratchpad: {}", addr_clone);
                                }
                                _ => {
                                    info!(
                                        "Scratchpad {} not found on network, already removed",
                                        addr_clone
                                    );
                                }
                            }
                        }
                    }
                    Ok::<(), Error>(())
                });
                all_futures.push(future);
            }
        }

        // Add upload pointer existence checks and operations - all concurrent
        for (address, target, key) in upload_pointer_data {
            let client = &self.client;
            let addr_clone = address.clone();
            let target_clone = target.clone();
            let payment_opt = payment_option.clone();

            let future = Box::pin(async move {
                let pointer_address = PointerAddress::from_hex(&addr_clone)?;
                let exists = client.pointer_get(&pointer_address).await.is_ok();

                if exists {
                    // Update existing pointer
                    let target_address = ScratchpadAddress::from_hex(&target_clone)?;
                    let target_obj = PointerTarget::ScratchpadAddress(target_address);
                    client.pointer_update(&key, target_obj).await?;
                    debug!("Successfully updated pointer: {}", addr_clone);
                } else {
                    // Create new pointer
                    let target_address = ScratchpadAddress::from_hex(&target_clone)?;
                    let pointer =
                        Pointer::new(&key, 0, PointerTarget::ScratchpadAddress(target_address));
                    client.pointer_put(pointer, payment_opt).await?;
                    debug!("Successfully created pointer: {}", addr_clone);
                }
                Ok::<(), Error>(())
            });
            all_futures.push(future);
        }

        // Add upload scratchpad existence checks and operations - all concurrent
        for (address, data, key) in upload_scratchpad_data {
            let client = &self.client;
            let addr_clone = address.clone();
            let data_clone = data.clone();
            let payment_opt = payment_option.clone();

            let future = Box::pin(async move {
                let scratchpad_address = ScratchpadAddress::from_hex(&addr_clone)?;
                let bytes = Bytes::from(data_clone.as_bytes().to_vec());

                match client.scratchpad_get(&scratchpad_address).await {
                    Ok(existing_scratchpad) => {
                        // Update existing scratchpad
                        let updated_scratchpad = Scratchpad::new_with_signature(
                            key.clone().public_key(),
                            0,
                            bytes.clone(),
                            existing_scratchpad.counter() + 1,
                            key.sign(Scratchpad::bytes_for_signature(
                                scratchpad_address,
                                0,
                                &bytes,
                                existing_scratchpad.counter() + 1,
                            )),
                        );
                        client
                            .scratchpad_put(updated_scratchpad, payment_opt.clone())
                            .await?;
                        debug!("Successfully updated scratchpad: {}", addr_clone);
                    }
                    Err(e) => {
                        match e {
                            ScratchpadError::Fork(scratchpads) => {
                                let existing_scratchpad =
                                    Self::select_newest_scratchpad(scratchpads);
                                // Update existing scratchpad
                                let updated_scratchpad = Scratchpad::new_with_signature(
                                    key.clone().public_key(),
                                    0,
                                    bytes.clone(),
                                    existing_scratchpad.counter() + 1,
                                    key.sign(Scratchpad::bytes_for_signature(
                                        scratchpad_address,
                                        0,
                                        &bytes,
                                        existing_scratchpad.counter() + 1,
                                    )),
                                );
                                client
                                    .scratchpad_put(updated_scratchpad, payment_opt.clone())
                                    .await?;
                                debug!("Successfully updated scratchpad: {}", addr_clone);
                            }
                            _ => {
                                // Create new scratchpad
                                let new_scratchpad = Scratchpad::new_with_signature(
                                    key.clone().public_key(),
                                    0,
                                    bytes.clone(),
                                    0,
                                    key.sign(Scratchpad::bytes_for_signature(
                                        scratchpad_address,
                                        0,
                                        &bytes,
                                        0,
                                    )),
                                );
                                client.scratchpad_put(new_scratchpad, payment_opt).await?;
                                debug!("Successfully created scratchpad: {}", addr_clone);
                            }
                        }
                    }
                }
                Ok::<(), Error>(())
            });
            all_futures.push(future);
        }

        // Execute ALL operations concurrently
        info!(
            "Executing {} total client operations concurrently",
            all_futures.len()
        );
        try_join_all(all_futures).await?;

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
        // Get the configuration address
        let configuration_address = self.key_store.get_configuration_address()?;
        let configuration_address = configuration_address.as_str();
        debug!(
            "Refreshing configuration address: {}",
            configuration_address
        );

        // Download the configuration pod pointer
        let pointer_address = PointerAddress::from_hex(configuration_address)?;
        let pointer = match self.client.pointer_get(&pointer_address).await {
            Ok(pointer) => pointer,
            Err(e) => {
                match e {
                    PointerError::CannotUpdateNewPointer => {
                        warn!("Configuration pointer not found on network, skipping");
                        return Ok(()); // Skip to the next pointer
                    }
                    // Catch Pointer(Network(GetRecordError(RecordNotFound))) error when there is nothing on the network
                    PointerError::GetError(GetError::RecordNotFound) => {
                        warn!("Configuration pointer not found on network, skipping");
                        return Ok(()); // Skip to the next pointer
                    }
                    _ => {
                        error!("Error occurred: {:?}", e); // Log the error
                        return Err(Error::Pointer(Box::new(e))); // Propagate the error to the higher-level function
                    }
                }
            }
        };

        debug!("Retrieved pointer. Update count: {}", pointer.counter());
        // FIXME: should look into a 'force' or 'override' argument to force a cache update if something is busted on the network
        // For now while the network is flakey, just always refresh because pointers fail, scratchpads fail, everything fails
        // Check if the pointer counter is newer than the local cache. If the pointer is older, we are done.
        // The MAX condition is the special case where the pointer file is not found and we always want to refresh
        // let local_pointer_count = self
        //     .data_store
        //     .get_pointer_count(configuration_address)
        //     .unwrap_or(u64::MAX);
        // if pointer.counter() as u64 <= local_pointer_count && local_pointer_count != u64::MAX {
        //     info!("Local pods are up to date, skipping refresh");
        //     return Ok(());
        // }

        // Check the configuration pod target
        let target = pointer.target();
        let target = match target {
            PointerTarget::ScratchpadAddress(scratchpad_address) => scratchpad_address,
            _ => {
                error!("Configuration pointer target is not a scratchpad address");
                return Ok(());
            }
        };
        debug!("Retrieved scratchpad address: {}", target.to_hex());

        // Download the configuration pod data using concurrent approach
        let scratchpad_operations = vec![(
            configuration_address.to_string(),
            *target,
            true,
            pointer.counter() as u64,
        )];
        self.execute_scratchpad_operations_concurrent(scratchpad_operations)
            .await?;
        debug!("Retrieved scratchpad data");

        // Update the configuration local pointer count value after the scratchpads are fetched
        self.data_store
            .update_pointer_count(configuration_address, pointer.counter())?;

        // When the configuration pointer fails to fetch, the file gets corrupted, so we need to update the target as well
        self.data_store
            .update_pointer_target(configuration_address, target.to_hex().as_str())?;

        // Get the list of used and free pointers and scratchpads from the graph
        let mut free_pointers = self.graph.get_free_pointers(configuration_address)?;
        let mut free_scratchpads = self.graph.get_free_scratchpads(configuration_address)?;
        let pointers = self.graph.get_pointers(configuration_address)?;
        let scratchpads = self.graph.get_scratchpads(configuration_address)?;
        // FIXME: should just remove this get_key_count() function because it tends to get corrupted. Use the
        // actual lists to determine the key count
        //let key_count = self.graph.get_key_count(configuration_address)?;
        let key_count =
            (free_pointers.len() + free_scratchpads.len() + pointers.len() + scratchpads.len())
                as u64;

        // Check if the update_list pods section contains any of the free pointers or scratchpads
        // If so, remove them from the free pointers and scratchpads lists
        let update_list = self.data_store.get_update_list()?;
        for (pod_address, scratchpad_addresses) in &update_list.pods {
            // Check if the pod address itself is in free_pointers
            if free_pointers.contains(pod_address) {
                free_pointers.retain(|x| x != pod_address);
            }

            // Check if the pod address itself is in free_scratchpads
            if free_scratchpads.contains(pod_address) {
                free_scratchpads.retain(|x| x != pod_address);
            }

            // Check each scratchpad address associated with this pod
            for scratchpad_address in scratchpad_addresses {
                if free_pointers.contains(scratchpad_address) {
                    free_pointers.retain(|x| x != scratchpad_address);
                }
                if free_scratchpads.contains(scratchpad_address) {
                    free_scratchpads.retain(|x| x != scratchpad_address);
                }
            }
        }

        // Remove the free pointers and scratchpads from the data store
        for pointer in free_pointers.clone() {
            self.data_store.remove_pointer_file(pointer.trim())?;
        }
        for scratchpad in free_scratchpads.clone() {
            self.data_store.remove_scratchpad_file(scratchpad.trim())?;
        }

        // Clear out the key store pointers, scratchpads, free_pointers, free_scratchpads, and bad_keys hashmaps
        self.key_store.clear_keys()?;

        // Walk through all of the derived keys up to the key count in the graph
        for i in 0..key_count {
            let address = self.key_store.get_address_at_index(i)?;
            // Check if the address matches any of the values in the pointers, scratchpads, free_pointers, or free_scratchpads vectors
            // If a match is found, map it to the proper key store hashmap
            // If a match is not found, add it to the bad_keys hashmap
            if pointers.contains(&address) {
                self.key_store.add_pointer_key()?;
            } else if scratchpads.contains(&address) {
                self.key_store.add_scratchpad_key()?;
            } else if free_pointers.contains(&address) {
                self.key_store.add_free_pointer_key()?;
            } else if free_scratchpads.contains(&address) {
                self.key_store.add_free_scratchpad_key()?;
            } else {
                self.key_store.add_bad_key()?;
            }
        }

        // Once the key store is updated, proceed with the normal refresh

        // Get the list of local pointers from the key store and process them with maximum concurrency
        let pointer_addresses: Vec<(String, PointerAddress)> = self
            .key_store
            .get_pointers()
            .into_keys()
            .filter_map(|address| {
                let address = address.trim().to_string();
                let pointer_address = PointerAddress::from_hex(&address);
                match pointer_address {
                    Ok(pa) => Some((address, pa)),
                    Err(e) => {
                        error!("Invalid pointer address {}: {:?}", address, e);
                        None
                    }
                }
            })
            .collect();

        info!(
            "Processing {} pointers with maximum concurrency",
            pointer_addresses.len()
        );

        // Execute all operations with maximum concurrency
        self.execute_refresh_cache_concurrent(pointer_addresses)
            .await?;
        Ok(())
    }

    /// Execute refresh cache operations with maximum concurrency - all client operations run simultaneously
    async fn execute_refresh_cache_concurrent(
        &mut self,
        pointer_addresses: Vec<(String, PointerAddress)>,
    ) -> Result<(), Error> {
        // Phase 1: Fetch all pointers concurrently
        info!(
            "Phase 1: Fetching {} pointers concurrently",
            pointer_addresses.len()
        );

        let pointer_futures: Vec<_> = pointer_addresses
            .iter()
            .map(|(address, pointer_address)| {
                let client = &self.client;
                let address = address.clone();
                async move {
                    info!("Checking pointer: {}", address);
                    let result = client.pointer_get(pointer_address).await;
                    (address, result)
                }
            })
            .collect();

        let pointer_results = join_all(pointer_futures).await;

        // Phase 2: Collect all scratchpad operations that need to be performed
        let mut scratchpad_operations: Vec<(String, ScratchpadAddress, bool, u64)> = Vec::new(); // (address, scratchpad_addr, pointer_exists, counter)

        for (address, pointer_result) in pointer_results {
            let pointer = match pointer_result {
                Ok(pointer) => pointer,
                Err(e) => match e {
                    PointerError::CannotUpdateNewPointer => {
                        warn!("Pointer not found on network, skipping: {}", address);
                        continue;
                    }
                    PointerError::GetError(GetError::RecordNotFound) => {
                        warn!("Pointer not found on network, skipping: {}", address);
                        continue;
                    }
                    _ => {
                        error!("Error occurred: {:?}", e);
                        return Err(Error::Pointer(Box::new(e)));
                    }
                },
            };

            info!("Pointer found: {:?}", pointer);

            // Check if the pointer file exists in the local data store
            let pointer_exists = self.data_store.address_is_pointer(&address)?;
            if !pointer_exists {
                info!("Pointer file does not exist, creating it");
                self.data_store.create_pointer_file(&address)?;
                self.data_store
                    .update_pointer_target(&address, pointer.target().to_hex().as_str())?;
                self.data_store
                    .update_pointer_count(&address, pointer.counter())?;
            }

            // Check if the pointer is newer than the local cache
            let local_pointer_count = self.data_store.get_pointer_count(&address)?;
            if (pointer.counter() as u64 > local_pointer_count) || !pointer_exists {
                info!("Pointer is newer, queuing scratchpad update");
                let target = match pointer.target() {
                    PointerTarget::ScratchpadAddress(scratchpad_address) => scratchpad_address,
                    _ => {
                        error!("Pointer target is not a scratchpad address, skipping");
                        continue;
                    }
                };

                scratchpad_operations.push((
                    address,
                    *target,
                    pointer_exists,
                    pointer.counter() as u64,
                ));
            } else {
                info!("Pointer is up to date");
            }
        }

        // Phase 3: Execute all scratchpad operations with maximum concurrency
        if !scratchpad_operations.is_empty() {
            info!(
                "Phase 3: Processing {} scratchpad operations with maximum concurrency",
                scratchpad_operations.len()
            );
            self.execute_scratchpad_operations_concurrent(scratchpad_operations)
                .await?;
        }

        Ok(())
    }

    /// Execute scratchpad operations with maximum concurrency - all client operations run simultaneously
    async fn execute_scratchpad_operations_concurrent(
        &mut self,
        scratchpad_operations: Vec<(String, ScratchpadAddress, bool, u64)>,
    ) -> Result<(), Error> {
        // Phase 1: Download main scratchpads concurrently to discover additional scratchpads
        info!(
            "Phase 1: Downloading {} main scratchpads concurrently",
            scratchpad_operations.len()
        );

        let main_scratchpad_futures: Vec<_> = scratchpad_operations
            .iter()
            .map(
                |(pod_address, scratchpad_address, _pointer_exists, _counter)| {
                    let client = &self.client;
                    let address = *scratchpad_address;
                    let pod_addr = pod_address.clone();
                    async move {
                        match client.scratchpad_get(&address).await {
                            Ok(scratchpad) => {
                                let data = scratchpad.encrypted_data();
                                let data_string = String::from_utf8(data.to_vec())?;
                                Ok((pod_addr, address.to_hex(), data_string))
                            }
                            Err(e) => match e {
                                ScratchpadError::Fork(scratchpads) => {
                                    let scratchpad = Self::select_newest_scratchpad(scratchpads);
                                    let data = scratchpad.encrypted_data();
                                    let data_string = String::from_utf8(data.to_vec())?;
                                    Ok((pod_addr, address.to_hex(), data_string))
                                }
                                _ => {
                                    info!(
                                        "Main scratchpad not found on network: {}",
                                        address.to_hex()
                                    );
                                    Err(Error::Scratchpad(Box::new(e)))
                                }
                            },
                        }
                    }
                },
            )
            .collect();

        let main_results = join_all(main_scratchpad_futures).await;

        // Phase 2: Collect all additional scratchpad addresses from main scratchpads
        let mut all_scratchpad_operations: Vec<(String, ScratchpadAddress, usize)> = Vec::new(); // (pod_address, scratchpad_addr, order_index)
        let mut pod_main_data: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        for result in main_results.into_iter() {
            match result {
                Ok((pod_address, scratchpad_hex, data_string)) => {
                    // Store main scratchpad data
                    pod_main_data.insert(pod_address.clone(), data_string.clone());

                    // Create scratchpad file if it doesn't exist
                    if !self.data_store.address_is_scratchpad(&scratchpad_hex)? {
                        info!("Scratchpad file does not exist, creating it");
                        self.data_store.create_scratchpad_file(&scratchpad_hex)?;
                    }
                    self.data_store
                        .update_scratchpad_data(&scratchpad_hex, data_string.trim())?;

                    // Parse to find additional scratchpads
                    if let Ok(scratchpads) = self
                        .graph
                        .get_pod_scratchpads_from_string(data_string.trim())
                    {
                        if scratchpads.len() > 1 {
                            // Add additional scratchpads (skip first one which is the main one we already have)
                            for (j, additional_address) in
                                scratchpads.into_iter().skip(1).enumerate()
                            {
                                if let Ok(addr) =
                                    ScratchpadAddress::from_hex(additional_address.trim())
                                {
                                    all_scratchpad_operations.push((
                                        pod_address.clone(),
                                        addr,
                                        j + 1,
                                    )); // +1 because main is index 0
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    // Skip failed main scratchpad downloads
                    continue;
                }
            }
        }

        // Phase 3: Download ALL additional scratchpads concurrently
        if !all_scratchpad_operations.is_empty() {
            info!(
                "Phase 3: Downloading {} additional scratchpads concurrently",
                all_scratchpad_operations.len()
            );

            let additional_futures: Vec<_> = all_scratchpad_operations
                .iter()
                .map(|(pod_address, scratchpad_address, order_index)| {
                    let client = &self.client;
                    let address = *scratchpad_address;
                    let pod_addr = pod_address.clone();
                    let index = *order_index;
                    async move {
                        // Create scratchpad file if it doesn't exist
                        let scratchpad_hex = address.to_hex();

                        match client.scratchpad_get(&address).await {
                            Ok(scratchpad) => {
                                let data = scratchpad.encrypted_data();
                                let data_string = String::from_utf8(data.to_vec())?;
                                Ok((pod_addr, scratchpad_hex, data_string, index))
                            }
                            Err(e) => match e {
                                ScratchpadError::Fork(scratchpads) => {
                                    let scratchpad = Self::select_newest_scratchpad(scratchpads);
                                    let data = scratchpad.encrypted_data();
                                    let data_string = String::from_utf8(data.to_vec())?;
                                    Ok((pod_addr, scratchpad_hex, data_string, index))
                                }
                                _ => {
                                    info!(
                                        "Additional scratchpad not found on network: {}",
                                        scratchpad_hex
                                    );
                                    Err(Error::Scratchpad(Box::new(e)))
                                }
                            },
                        }
                    }
                })
                .collect();

            let additional_results = join_all(additional_futures).await;

            // Phase 4: Group additional scratchpad data by pod and sort by order
            let mut pod_additional_data: std::collections::HashMap<
                String,
                Vec<(usize, String, String)>,
            > = std::collections::HashMap::new();

            for result in additional_results {
                match result {
                    Ok((pod_address, scratchpad_hex, data_string, order_index)) => {
                        // Create scratchpad file if it doesn't exist
                        if !self.data_store.address_is_scratchpad(&scratchpad_hex)? {
                            info!("Scratchpad file does not exist, creating it");
                            self.data_store.create_scratchpad_file(&scratchpad_hex)?;
                        }
                        self.data_store
                            .update_scratchpad_data(&scratchpad_hex, data_string.trim())?;

                        pod_additional_data.entry(pod_address).or_default().push((
                            order_index,
                            scratchpad_hex,
                            data_string,
                        ));
                    }
                    Err(_) => {
                        // Skip failed additional scratchpad downloads
                        continue;
                    }
                }
            }

            // Phase 5: Combine all data for each pod in correct order
            for (pod_address, main_data) in pod_main_data {
                let mut combined_data = main_data;

                if let Some(mut additional_data) = pod_additional_data.remove(&pod_address) {
                    // Sort additional data by order index to preserve order
                    additional_data.sort_by_key(|(order_index, _, _)| *order_index);

                    // Append additional data in order
                    for (_, _, data_string) in additional_data {
                        combined_data.push_str(&data_string);
                    }
                }

                // Load the pod data into the graph database
                info!("Loading pod into graph database: {}", pod_address);
                if !combined_data.trim().is_empty() {
                    if let Err(e) = self.load_pod_into_graph(&pod_address, combined_data.trim()) {
                        warn!(
                            "Failed to load pod data into graph for {}: {}",
                            pod_address, e
                        );
                    }
                }

                // Set the depth attribute to 0 (local pod)
                if let Err(e) = self.update_pod_depth(&pod_address, 0) {
                    warn!("Failed to update pod depth for {}: {}", pod_address, e);
                }

                info!(
                    "Successfully updated graph database for pod: {}",
                    pod_address
                );
            }
        } else {
            // No additional scratchpads, just process main data
            for (pod_address, main_data) in pod_main_data {
                // Load the pod data into the graph database
                info!("Loading pod into graph database: {}", pod_address);
                if !main_data.trim().is_empty() {
                    if let Err(e) = self.load_pod_into_graph(&pod_address, main_data.trim()) {
                        warn!(
                            "Failed to load pod data into graph for {}: {}",
                            pod_address, e
                        );
                    }
                }

                // Set the depth attribute to 0 (local pod)
                if let Err(e) = self.update_pod_depth(&pod_address, 0) {
                    warn!("Failed to update pod depth for {}: {}", pod_address, e);
                }

                info!(
                    "Successfully updated graph database for pod: {}",
                    pod_address
                );
            }
        }

        Ok(())
    }

    /// Execute download scratchpad operations with maximum concurrency - all client operations run simultaneously
    async fn execute_download_scratchpad_operations_concurrent(
        &mut self,
        scratchpad_operations: Vec<(String, ScratchpadAddress, bool, u64)>,
        depth: u64,
    ) -> Result<(), Error> {
        // Phase 1: Download main scratchpads concurrently to discover additional scratchpads
        info!(
            "Phase 1: Downloading {} main scratchpads concurrently",
            scratchpad_operations.len()
        );

        let main_scratchpad_futures: Vec<_> = scratchpad_operations
            .iter()
            .map(
                |(pod_address, scratchpad_address, _pointer_exists, counter)| {
                    let client = &self.client;
                    let address = *scratchpad_address;
                    let pod_addr = pod_address.clone();
                    let pod_counter = *counter;
                    async move {
                        match client.scratchpad_get(&address).await {
                            Ok(scratchpad) => {
                                let data = scratchpad.encrypted_data();
                                let data_string = String::from_utf8(data.to_vec())?;
                                Ok((pod_addr, address.to_hex(), data_string, pod_counter))
                            }
                            Err(e) => match e {
                                ScratchpadError::Fork(scratchpads) => {
                                    let scratchpad = Self::select_newest_scratchpad(scratchpads);
                                    let data = scratchpad.encrypted_data();
                                    let data_string = String::from_utf8(data.to_vec())?;
                                    Ok((pod_addr, address.to_hex(), data_string, pod_counter))
                                }
                                _ => {
                                    info!(
                                        "Main scratchpad not found on network: {}",
                                        address.to_hex()
                                    );
                                    Err(Error::Scratchpad(Box::new(e)))
                                }
                            },
                        }
                    }
                },
            )
            .collect();

        let main_results = join_all(main_scratchpad_futures).await;

        // Phase 2: Collect all additional scratchpad addresses from main scratchpads
        let mut all_scratchpad_operations: Vec<(String, ScratchpadAddress, usize, u64)> =
            Vec::new(); // (pod_address, scratchpad_addr, order_index, counter)
        let mut pod_main_data: std::collections::HashMap<String, (String, u64, String)> =
            std::collections::HashMap::new(); // (data, counter, main_scratchpad_hex)

        for result in main_results.into_iter() {
            match result {
                Ok((pod_address, scratchpad_hex, data_string, counter)) => {
                    // Store main scratchpad data with scratchpad address
                    pod_main_data.insert(
                        pod_address.clone(),
                        (data_string.clone(), counter, scratchpad_hex.clone()),
                    );

                    // Create scratchpad file if it doesn't exist
                    if !self.data_store.address_is_scratchpad(&scratchpad_hex)? {
                        info!("Scratchpad file does not exist, creating it");
                        self.data_store.create_scratchpad_file(&scratchpad_hex)?;
                    }
                    self.data_store
                        .update_scratchpad_data(&scratchpad_hex, data_string.trim())?;

                    // Parse to find additional scratchpads
                    if let Ok(scratchpads) = self
                        .graph
                        .get_pod_scratchpads_from_string(data_string.trim())
                    {
                        if scratchpads.len() > 1 {
                            // Add additional scratchpads (skip first one which is the main one we already have)
                            for (j, additional_address) in
                                scratchpads.into_iter().skip(1).enumerate()
                            {
                                if let Ok(addr) =
                                    ScratchpadAddress::from_hex(additional_address.trim())
                                {
                                    all_scratchpad_operations.push((
                                        pod_address.clone(),
                                        addr,
                                        j + 1,
                                        counter,
                                    )); // +1 because main is index 0
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    // Skip failed main scratchpad downloads
                    continue;
                }
            }
        }

        // Phase 3: Download ALL additional scratchpads concurrently
        if !all_scratchpad_operations.is_empty() {
            info!(
                "Phase 3: Downloading {} additional scratchpads concurrently",
                all_scratchpad_operations.len()
            );

            let additional_futures: Vec<_> = all_scratchpad_operations
                .iter()
                .map(|(pod_address, scratchpad_address, order_index, counter)| {
                    let client = &self.client;
                    let address = *scratchpad_address;
                    let pod_addr = pod_address.clone();
                    let index = *order_index;
                    let pod_counter = *counter;
                    async move {
                        match client.scratchpad_get(&address).await {
                            Ok(scratchpad) => {
                                let data = scratchpad.encrypted_data();
                                let data_string = String::from_utf8(data.to_vec())?;
                                Ok((pod_addr, address.to_hex(), data_string, index, pod_counter))
                            }
                            Err(e) => match e {
                                ScratchpadError::Fork(scratchpads) => {
                                    let scratchpad = Self::select_newest_scratchpad(scratchpads);
                                    let data = scratchpad.encrypted_data();
                                    let data_string = String::from_utf8(data.to_vec())?;
                                    Ok((
                                        pod_addr,
                                        address.to_hex(),
                                        data_string,
                                        index,
                                        pod_counter,
                                    ))
                                }
                                _ => {
                                    info!(
                                        "Additional scratchpad not found on network: {}",
                                        address.to_hex()
                                    );
                                    Err(Error::Scratchpad(Box::new(e)))
                                }
                            },
                        }
                    }
                })
                .collect();

            let additional_results = join_all(additional_futures).await;

            // Phase 4: Group additional scratchpad data by pod and sort by order
            let mut pod_additional_data: std::collections::HashMap<
                String,
                Vec<(usize, String, String)>,
            > = std::collections::HashMap::new();

            for result in additional_results {
                match result {
                    Ok((pod_address, scratchpad_hex, data_string, order_index, _counter)) => {
                        // Create scratchpad file if it doesn't exist
                        if !self.data_store.address_is_scratchpad(&scratchpad_hex)? {
                            info!("Scratchpad file does not exist, creating it");
                            self.data_store.create_scratchpad_file(&scratchpad_hex)?;
                        }
                        self.data_store
                            .update_scratchpad_data(&scratchpad_hex, data_string.trim())?;

                        pod_additional_data.entry(pod_address).or_default().push((
                            order_index,
                            scratchpad_hex,
                            data_string,
                        ));
                    }
                    Err(_) => {
                        // Skip failed additional scratchpad downloads
                        continue;
                    }
                }
            }

            // Phase 5: Combine all data for each pod in correct order and update metadata
            for (pod_address, (main_data, counter, main_scratchpad_hex)) in pod_main_data {
                let mut combined_data = main_data;

                if let Some(mut additional_data) = pod_additional_data.remove(&pod_address) {
                    // Sort additional data by order index to preserve order
                    additional_data.sort_by_key(|(order_index, _, _)| *order_index);

                    // Append additional data in order
                    for (_, _, data_string) in additional_data {
                        combined_data.push_str(&data_string);
                    }
                }

                // Load the pod data into the graph database
                info!(
                    "Loading referenced pod into graph database: {}",
                    pod_address
                );
                if !combined_data.trim().is_empty() {
                    if let Err(e) = self.load_pod_into_graph(&pod_address, combined_data.trim()) {
                        warn!(
                            "Failed to load pod data into graph for {}: {}",
                            pod_address, e
                        );
                    }
                }

                // Update pointer information - CRITICAL: Store the scratchpad address in pointer file
                if let Err(e) = self
                    .data_store
                    .update_pointer_target(&pod_address, &main_scratchpad_hex)
                {
                    warn!("Failed to update pointer target for {}: {}", pod_address, e);
                }
                if let Err(e) = self.data_store.update_pointer_count(&pod_address, counter) {
                    warn!("Failed to update pointer count for {}: {}", pod_address, e);
                }

                // Set the depth attribute
                if let Err(e) = self.update_pod_depth(&pod_address, depth) {
                    warn!("Failed to update pod depth for {}: {}", pod_address, e);
                }

                info!("Successfully downloaded referenced pod: {}", pod_address);
            }
        } else {
            // No additional scratchpads, just process main data
            for (pod_address, (main_data, counter, main_scratchpad_hex)) in pod_main_data {
                // Load the pod data into the graph database
                info!(
                    "Loading referenced pod into graph database: {}",
                    pod_address
                );
                if !main_data.trim().is_empty() {
                    if let Err(e) = self.load_pod_into_graph(&pod_address, main_data.trim()) {
                        warn!(
                            "Failed to load pod data into graph for {}: {}",
                            pod_address, e
                        );
                    }
                }

                // Update pointer information - CRITICAL: Store the scratchpad address in pointer file
                if let Err(e) = self
                    .data_store
                    .update_pointer_target(&pod_address, &main_scratchpad_hex)
                {
                    warn!("Failed to update pointer target for {}: {}", pod_address, e);
                }
                if let Err(e) = self.data_store.update_pointer_count(&pod_address, counter) {
                    warn!("Failed to update pointer count for {}: {}", pod_address, e);
                }

                // Set the depth attribute
                if let Err(e) = self.update_pod_depth(&pod_address, depth) {
                    warn!("Failed to update pod depth for {}: {}", pod_address, e);
                }

                info!("Successfully downloaded referenced pod: {}", pod_address);
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
    ///   - `0`: Recurse through all pods until there is nothing left to download
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
        self.refresh_cache().await?;

        // Process pods iteratively up to the specified depth to avoid async recursion
        let mut all_processed_pods = std::collections::HashSet::new();
        let mut current_depth: u64 = 0;

        loop {
            info!("Processing pod references at depth {}", current_depth);

            // Get all pods at the current depth
            let pod_addresses = self.get_pods_at_depth(current_depth)?;
            let mut newly_downloaded_pods: Vec<String> = Vec::new();

            // Collect all pod references that need to be downloaded
            let mut pod_refs_to_download: Vec<String> = Vec::new();

            // Walk through each pod graph and check if it references other pods
            for pod_address in &pod_addresses {
                // Skip if we've already processed this pod
                if all_processed_pods.contains(pod_address) {
                    continue;
                }

                info!("Checking pod {} for references", pod_address);
                let pod_refs = self.get_pod_references(pod_address)?;

                for pod_ref in pod_refs {
                    // Check if the pod_ref has already been processed
                    if all_processed_pods.contains(&pod_ref) {
                        info!("Pod reference {} already processed, skipping", pod_ref);
                        continue;
                    }

                    info!("Queuing referenced pod for download: {}", pod_ref);
                    pod_refs_to_download.push(pod_ref);
                }
            }

            // Download all referenced pods using optimized batch download
            if !pod_refs_to_download.is_empty() {
                info!(
                    "Downloading {} referenced pods with concurrent network operations",
                    pod_refs_to_download.len()
                );

                let successful_downloads = self
                    .download_referenced_pods_batch(&pod_refs_to_download, current_depth + 1)
                    .await?;
                newly_downloaded_pods.extend(successful_downloads);
            }

            // Mark all processed pods
            for pod_address in pod_addresses {
                all_processed_pods.insert(pod_address);
            }

            // Process newly downloaded pods for their references in the same iteration
            let mut pods_to_process = newly_downloaded_pods;
            while !pods_to_process.is_empty() {
                let mut pod_refs_to_download: Vec<String> = Vec::new();

                // Collect all references from newly downloaded pods
                for pod_address in &pods_to_process {
                    // Skip if we've already processed this pod
                    if all_processed_pods.contains(pod_address) {
                        continue;
                    }

                    info!(
                        "Checking newly downloaded pod {} for references",
                        pod_address
                    );
                    let pod_refs = self.get_pod_references(pod_address)?;

                    for pod_ref in pod_refs {
                        // Check if the pod_ref has already been processed
                        if all_processed_pods.contains(&pod_ref) {
                            info!("Pod reference {} already processed, skipping", pod_ref);
                            continue;
                        }

                        info!("Queuing referenced pod from newly downloaded: {}", pod_ref);
                        pod_refs_to_download.push(pod_ref);
                    }
                }

                // Download all referenced pods using batch download
                let next_batch = if !pod_refs_to_download.is_empty() {
                    info!(
                        "Batch downloading {} referenced pods from newly downloaded pods",
                        pod_refs_to_download.len()
                    );
                    self.download_referenced_pods_batch(&pod_refs_to_download, current_depth + 1)
                        .await?
                } else {
                    Vec::new()
                };

                // Mark all processed pods
                for pod_address in pods_to_process {
                    all_processed_pods.insert(pod_address);
                }

                pods_to_process = next_batch;
            }

            // Check if we should continue to the next depth
            if depth > 0 && current_depth >= depth {
                info!("Reached specified depth {}, stopping processing", depth);
                break;
            }

            // Check if there are any pods at the next depth
            let next_depth_pods = self.get_pods_at_depth(current_depth + 1)?;
            if next_depth_pods.is_empty() {
                info!("No more pods found at deeper levels, stopping processing");
                break;
            }

            current_depth += 1;
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

    // Download multiple referenced pods with maximum concurrent network operations
    async fn download_referenced_pods_batch(
        &mut self,
        pod_addresses: &[String],
        depth: u64,
    ) -> Result<Vec<String>, Error> {
        info!(
            "Batch downloading {} referenced pods at depth {} with maximum concurrency",
            pod_addresses.len(),
            depth
        );

        // Execute all operations with maximum concurrency
        self.execute_download_pods_concurrent(pod_addresses, depth)
            .await
    }

    /// Execute download operations with maximum concurrency - all client operations run simultaneously
    async fn execute_download_pods_concurrent(
        &mut self,
        pod_addresses: &[String],
        depth: u64,
    ) -> Result<Vec<String>, Error> {
        // Phase 1: Fetch all pointers concurrently
        info!(
            "Phase 1: Fetching {} pointers concurrently",
            pod_addresses.len()
        );

        let pointer_futures: Vec<_> = pod_addresses
            .iter()
            .map(|pod_address| {
                let client = &self.client;
                let address = pod_address.clone();
                async move {
                    let pointer_address = PointerAddress::from_hex(&address)?;
                    let result = client.pointer_get(&pointer_address).await;
                    Ok::<(String, Result<Pointer, PointerError>), Error>((address, result))
                }
            })
            .collect();

        let pointer_results = try_join_all(pointer_futures).await?;

        // Phase 2: Process results and collect operations to perform
        let mut pods_to_download: Vec<(String, Pointer)> = Vec::new();
        let mut successful_downloads: Vec<String> = Vec::new();

        for (pod_address, pointer_result) in pointer_results {
            let pointer = match pointer_result {
                Ok(pointer) => pointer,
                Err(e) => {
                    match e {
                        PointerError::CannotUpdateNewPointer => {
                            warn!("Referenced pod not found on network: {}", pod_address);
                            continue; // Skip this pod if it doesn't exist
                        }
                        PointerError::GetError(GetError::RecordNotFound) => {
                            warn!("Referenced pod not found on network: {}", pod_address);
                            continue; // Skip this pod if it doesn't exist
                        }
                        _ => {
                            error!("Error occurred for pod {}: {:?}", pod_address, e);
                            continue; // Skip this pod on error
                        }
                    }
                }
            };

            // Check if we need to download this pod
            let pod_exists = self.data_store.address_is_pointer(&pod_address)?;
            let should_download = if pod_exists {
                let local_pointer_count = self.data_store.get_pointer_count(&pod_address)?;
                let remote_counter = pointer.counter() as u64;
                if remote_counter > local_pointer_count {
                    info!(
                        "Remote pod is newer (counter: {} > {}), queuing for download: {}",
                        remote_counter, local_pointer_count, pod_address
                    );
                    true
                } else {
                    info!(
                        "Local pod is up to date (counter: {} >= {}), skipping: {}",
                        local_pointer_count, remote_counter, pod_address
                    );
                    false
                }
            } else {
                info!(
                    "Pod doesn't exist locally, queuing for download: {}",
                    pod_address
                );
                self.data_store.create_pointer_file(&pod_address)?;
                true
            };

            if should_download {
                pods_to_download.push((pod_address.clone(), pointer));
            }

            // Always update depth and mark as processed
            self.update_pod_depth(&pod_address, depth)?;
            successful_downloads.push(pod_address);
        }

        // Phase 3: Download scratchpad data for pods that need it with maximum concurrency
        if !pods_to_download.is_empty() {
            info!(
                "Phase 3: Processing {} pod downloads with maximum concurrency",
                pods_to_download.len()
            );

            // Convert to scratchpad operations format
            let mut scratchpad_operations = Vec::new();
            for (pod_address, pointer) in pods_to_download {
                let target = match pointer.target() {
                    PointerTarget::ScratchpadAddress(scratchpad_address) => scratchpad_address,
                    _ => {
                        error!(
                            "Pointer target is not a scratchpad address for pod: {}",
                            pod_address
                        );
                        continue;
                    }
                };

                scratchpad_operations.push((pod_address, *target, true, pointer.counter()));
            }

            // Execute scratchpad downloads with maximum concurrency
            self.execute_download_scratchpad_operations_concurrent(scratchpad_operations, depth)
                .await?;
        }

        Ok(successful_downloads)
    }

    // Update the depth attribute of a pod in the graph database
    fn update_pod_depth(&mut self, pod_address: &str, depth: u64) -> Result<(), Error> {
        // Get the configuration address
        let configuration_address = self.key_store.get_configuration_address()?;
        let configuration_address = configuration_address.as_str();

        // Use the graph database to update the pod depth
        self.graph
            .update_pod_depth(pod_address, configuration_address, depth)?;
        Ok(())
    }

    // Load pod data into the graph database
    fn load_pod_into_graph(&mut self, pod_address: &str, pod_data: &str) -> Result<(), Error> {
        // The pod data should be in TriG format
        // Load it into the graph database using the Graph's method

        match self.graph.load_pod_into_graph(pod_address, pod_data) {
            Ok(_) => {
                info!(
                    "Successfully loaded pod {} data into graph database",
                    pod_address
                );
            }
            Err(e) => {
                warn!(
                    "Failed to load pod {} data into graph database: {}",
                    pod_address, e
                );
                // Don't fail the entire operation if graph loading fails
            }
        }

        Ok(())
    }
    /// Returns the current update list in JSON format
    ///
    /// # Returns
    ///
    /// Returns a JSON object containing the update list, or an `Error` if:
    /// - The update list cannot be read from local storage
    /// - JSON serialization fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// let update_list = pod_manager.get_update_list()?;
    /// println!("Update list: {}", serde_json::to_string_pretty(&update_list)?);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`upload_all`] - Uploads all pending changes to the network
    /// - [`refresh_cache`] - Downloads updates from the network
    /// - [`refresh_ref`] - Downloads referenced pods from the network
    /// - [`add_pod`] - Creates a new pod that needs uploading
    /// - [`put_subject_data`] - Adds data to a pod that needs uploading
    /// - [`remove_pod`] - Removes a pod that needs uploading
    /// - [`rename_pod`] - Renames a pod that needs uploading
    /// - [`add_pod_ref`] - Adds a pod reference that needs uploading
    /// - [`remove_pod_ref`] - Removes a pod reference that needs uploading
    ///
    /// # JSON Format
    ///
    /// The returned JSON object has the following structure:
    /// ```json
    /// {
    ///     "update": ["pod_address_1", "pod_address_2", ...],
    ///     "remove": ["pod_address_3", "pod_address_4", ...]
    /// }
    /// ```
    ///
    /// The "update" array contains addresses of pods that need to be updated or created on the network.
    /// The "remove" array contains addresses of pods that need to be removed from the network.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The update list cannot be read from local storage
    /// - JSON serialization fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// let update_list = pod_manager.get_update_list()?;
    /// println!("Update list: {}", serde_json::to_string_pretty(&update_list)?);
    /// # Ok(())
    /// # }
    /// ```
    ///
    pub fn get_update_list(&self) -> Result<Value, Error> {
        let update_list = self.data_store.get_update_list()?;

        // Restructure the update list to match the expected JSON format
        // There are 2 main keys: "update" and "remove"
        // "update" contains an array of pod addresses to update or create
        // "remove" contains an array of pod address to remove

        let mut update_list_json = serde_json::Map::new();
        let mut update_array = Vec::new();
        for pod_address in update_list.pods.keys() {
            update_array.push(Value::String(pod_address.to_string()));
        }
        update_list_json.insert("update".to_string(), Value::Array(update_array));
        let mut remove_array = Vec::new();
        for pod_address in update_list.remove.pointers {
            remove_array.push(Value::String(pod_address));
        }
        update_list_json.insert("remove".to_string(), Value::Array(remove_array));
        Ok(Value::Object(update_list_json))
    }

    /// Adds a new wallet key to the key store with the specified name.
    ///
    /// This function stores a wallet private key in the key store using a human-readable name
    /// as the identifier. The wallet key is stored in hexadecimal format and can be retrieved
    /// later using the same name. Multiple wallet keys can be stored with different names.
    ///
    /// # Parameters
    ///
    /// * `name` - A string identifier for the wallet key (e.g., "main", "backup", "testnet")
    /// * `wallet_key` - The private key as a hexadecimal string (with or without "0x" prefix)
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the wallet key was successfully added to the key store.
    ///
    /// Returns an `Error` if:
    /// - The wallet key is not a valid hexadecimal string
    /// - The key store update fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // Add a main wallet key
    /// let main_key = "0x1234512345123451234512345123451234512345123451234512345123451234";
    /// pod_manager.add_wallet_key("main", main_key).await?;
    ///
    /// // Add a backup wallet key
    /// let backup_key = "abcdabcde12345abcde12345abcde12345abcde12345abcde12345eabcde12345";
    /// pod_manager.add_wallet_key("backup", backup_key).await?;
    ///
    /// // Add a testnet wallet key
    /// let testnet_key = "0x1234567890123456789012345678901234567890123456789012345678901234";
    /// pod_manager.add_wallet_key("testnet", testnet_key).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`get_wallet_key`] - Retrieve a wallet key by name
    /// - [`get_wallet_keys`] - Retrieve all wallet keys
    /// - [`remove_wallet_key`] - Remove a wallet key by name
    pub async fn add_wallet_key(&mut self, name: &str, wallet_key: &str) -> Result<(), Error> {
        self.key_store.add_wallet_key(name, wallet_key)?;
        Ok(())
    }

    /// Retrieves a wallet key from the key store by name.
    ///
    /// This function looks up a previously stored wallet key using its name identifier
    /// and returns the private key as a hexadecimal string. The key must have been
    /// previously added using the `add_wallet_key` function.
    ///
    /// # Parameters
    ///
    /// * `name` - The string identifier for the wallet key to retrieve
    ///
    /// # Returns
    ///
    /// Returns `Ok(String)` containing the wallet private key as a hexadecimal string.
    ///
    /// Returns an `Error` if:
    /// - No wallet key exists with the specified name
    /// - The key store lookup fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // First add a wallet key
    /// let main_key = "0x1234512345123451234512345123451234512345123451234512345123451234";
    /// pod_manager.add_wallet_key("main", main_key).await?;
    ///
    /// // Later retrieve the wallet key
    /// let retrieved_key = pod_manager.get_wallet_key("main").await?;
    /// assert_eq!(retrieved_key, "");
    ///
    /// // Try to get a non-existent key (will return an error)
    /// match pod_manager.get_wallet_key("nonexistent").await {
    ///     Ok(_) => panic!("Should not find non-existent key"),
    ///     Err(_) => println!("Key not found as expected"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`add_wallet_key`] - Add a new wallet key with a name
    /// - [`remove_wallet_key`] - Remove a wallet key by name
    /// - [`get_wallet_keys`] - Retrieve all wallet keys
    pub async fn get_wallet_key(&self, name: &str) -> Result<String, Error> {
        let key = self.key_store.get_wallet_key(name)?;
        Ok(key)
    }

    /// Sets the active wallet for the pod manager and persists it to local storage.
    ///
    /// This function designates a specific wallet as the active wallet for pod operations.
    /// The active wallet information is stored both in the key store and persisted to the
    /// `active_wallet.json` file in the local data directory for persistence across sessions.
    /// The wallet must already exist in the key store before it can be set as active.
    ///
    /// # Parameters
    ///
    /// * `name` - The name of the wallet to set as active (must exist in the key store)
    ///
    /// # Returns
    ///
    /// Returns `Ok((name, address))` containing:
    /// - `name` - The confirmed name of the active wallet
    /// - `address` - The Ethereum address of the active wallet
    ///
    /// Returns an `Error` if:
    /// - The wallet name does not exist in the key store
    /// - Writing to the active_wallet.json file fails
    /// - Key store operations fail
    ///
    /// # Side Effects
    ///
    /// - Updates the active wallet state in the key store
    /// - Creates or updates the `active_wallet.json` file in the data directory
    /// - The active wallet persists across application restarts
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // First add a wallet key to the key store
    /// pod_manager.add_wallet_key("main", "0x1234...").await?;
    ///
    /// // Set it as the active wallet
    /// let (name, address) = pod_manager.set_active_wallet("main")?;
    /// println!("Active wallet: {} at address {}", name, address);
    ///
    /// // The active wallet is now persisted and will be remembered on restart
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`get_active_wallet`] - Retrieve the current active wallet
    /// - [`add_wallet_key`] - Add a new wallet key to the key store
    /// - [`get_wallet_keys`] - List all available wallet keys
    pub fn set_active_wallet(&mut self, name: &str) -> Result<(String, String), Error> {
        let (name, address) = self.key_store.set_active_wallet(name)?;
        self.data_store.set_active_wallet(&name, &address)?;
        Ok((name, address))
    }

    /// Retrieves the currently active wallet from local storage.
    ///
    /// This function reads the active wallet information from the `active_wallet.json` file
    /// in the local data directory. The active wallet is the wallet that was previously set
    /// using `set_active_wallet()` and persists across application sessions.
    ///
    /// # Returns
    ///
    /// Returns `Ok((name, address))` containing:
    /// - `name` - The name of the active wallet
    /// - `address` - The Ethereum address of the active wallet
    ///
    /// Returns an `Error` if:
    /// - The `active_wallet.json` file does not exist (no active wallet has been set)
    /// - The file cannot be read due to permissions or I/O errors
    /// - The file contains invalid JSON or is corrupted
    /// - Required fields (name or address) are missing from the JSON
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // Check if there's an active wallet
    /// match pod_manager.get_active_wallet() {
    ///     Ok((name, address)) => {
    ///         println!("Current active wallet: {} at address {}", name, address);
    ///     }
    ///     Err(_) => {
    ///         println!("No active wallet set. Use set_active_wallet() to set one.");
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # File Format
    ///
    /// The `active_wallet.json` file contains:
    /// ```json
    /// {
    ///   "name": "wallet_name",
    ///   "address": "0x1234567890abcdef..."
    /// }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`set_active_wallet`] - Set a wallet as the active wallet
    /// - [`add_wallet_key`] - Add a new wallet key to the key store
    /// - [`get_wallet_keys`] - List all available wallet keys
    pub fn get_active_wallet(&self) -> Result<(String, String), Error> {
        let (name, address) = self.data_store.get_active_wallet()?;
        Ok((name, address))
    }

    /// Retrieves all wallet keys from the key store.
    ///
    /// # Returns
    ///
    /// Returns a `HashMap` containing all wallet keys, with the key name as the key and the hexadecimal key as the value.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // First add a wallet key
    /// let main_key = "0x1234512345123451234512345123451234512345123451234512345123451234";
    /// pod_manager.add_wallet_key("main", main_key).await?;
    ///
    /// // Later retrieve all wallet keys
    /// let wallet_keys = pod_manager.get_wallet_keys();
    /// for (name, key) in wallet_keys {
    ///     println!("Wallet key '{}' has value: {}", name, key);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    ///
    /// # Related Functions
    ///
    /// - [`add_wallet_key`] - Add a new wallet key with a name
    /// - [`get_wallet_key`] - Retrieve a specific wallet key by name
    /// - [`remove_wallet_key`] - Remove a wallet key by name
    pub fn get_wallet_keys(&self) -> HashMap<String, String> {
        self.key_store.get_wallet_keys()
    }

    /// Retrieves all wallet addresses from the key store.
    ///
    /// # Returns
    ///
    /// Returns a `HashMap` containing all wallet addresses, with the key name as the key and the hexadecimal address as the value.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // First add a wallet key
    /// let main_key = "0x1234512345123451234512345123451234512345123451234512345123451234";
    /// pod_manager.add_wallet_key("main", main_key).await?;
    ///
    /// // Later retrieve all wallet addresses
    /// let wallet_addresses = pod_manager.get_wallet_addresses();
    /// for (name, address) in wallet_addresses {
    ///     println!("Wallet address for '{}' is: {}", name, address);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`add_wallet_key`] - Add a new wallet key with a name
    /// - [`get_wallet_key`] - Retrieve a specific wallet key by name
    /// - [`get_wallet_keys`] - Retrieve all wallet keys
    /// - [`remove_wallet_key`] - Remove a wallet key by name
    /// - [`get_wallet_keys`] - Retrieve all wallet keys
    pub fn get_wallet_addresses(&self) -> HashMap<String, String> {
        self.key_store.get_wallet_addresses()
    }

    /// Removes a wallet key from the key store by name.
    ///
    /// # Parameters
    ///
    /// * `name` - The string identifier for the wallet key to remove
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the wallet key was successfully removed from the key store.
    ///
    /// Returns an `Error` if:
    /// - No wallet key exists with the specified name
    /// - The key store update fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// # async fn example(pod_manager: &mut PodManager<'_>) -> Result<(), Box<dyn std::error::Error>> {
    /// // First add a wallet key
    /// let main_key = "0x1234512345123451234512345123451234512345123451234512345123451234";
    /// pod_manager.add_wallet_key("main", main_key).await?;
    ///
    /// // Later remove the wallet key
    /// pod_manager.remove_wallet_key("main").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Functions
    ///
    /// - [`add_wallet_key`] - Add a new wallet key with a name
    /// - [`get_wallet_key`] - Retrieve a specific wallet key by name
    /// - [`get_wallet_keys`] - Retrieve all wallet keys
    pub fn remove_wallet_key(&mut self, name: &str) -> Result<(), Error> {
        self.key_store.remove_wallet_key(name)?;
        Ok(())
    }
}
