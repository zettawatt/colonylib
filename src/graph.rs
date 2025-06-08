use oxigraph::io::{RdfFormat, RdfParser, RdfParseError};
use tracing::{info, debug, error};
use thiserror;
use serde;
use oxigraph::sparql::{EvaluationError,QueryResults};
use oxigraph::model::{NamedNodeRef, IriParseError, QuadRef, TermRef, GraphNameRef, LiteralRef, Quad};
use oxigraph::store::{SerializerError, StorageError, Store, LoaderError};
use oxigraph::sparql::results::QueryResultsFormat;
use oxttl::TriGParser;
use std::path::PathBuf;
use serde_json::{Error as SerdeError};
use alloc::string::FromUtf8Error;
use chrono::Utc;
use oxjsonld::{JsonLdProfile, JsonLdProfileSet};
use std::io::Cursor;
use std::collections::HashMap;

//////////////////////////////////////////////
// Vocabulary
//////////////////////////////////////////////
macro_rules! PREDICATE {
    ($e:expr) => {
        concat!("ant://colonylib/vocabulary/", "0.1/", "predicate#", $e)
    };
}

macro_rules! OBJECT {
    ($e:expr) => {
        concat!("ant://colonylib/vocabulary/", "0.1/", "object#", $e)
    };
}

//////////////////////////////////////////////
// Predicates
//////////////////////////////////////////////

/// Address Type
/// Defines the type of the resource at the address
/// Object must be one of the address type objects
const HAS_ADDR_TYPE: &str = PREDICATE!("addr_type");

/// Name
/// The name of the resource
/// Object is a string literal
const HAS_NAME: &str = PREDICATE!("name");

/// Pod Depth
/// The depth of the pod in the reference tree
/// Only valid for POD and POD_REF address types
/// Object is a literal representing the depth, local pods are set to 0
/// This is a local attribute, not written out in the TriG format
const HAS_DEPTH: &str = PREDICATE!("depth");

/// Pod Index
/// The index for a pod scratchpad used to build up a pod from multiple scratchpads
/// Object is a literal representing the index
const HAS_POD_INDEX: &str = PREDICATE!("pod_index");

/// Date
/// The date when the resource was created or modified
/// Object is a literal representing the date
const HAS_DATE: &str = PREDICATE!("date");

//////////////////////////////////////////////
// Objects
//////////////////////////////////////////////

/// Address Type Objects
/// Defines what kind of object the address is pointing to
const POD: &str = OBJECT!("pod");
const POD_REF: &str = OBJECT!("pod_ref");

// Error handling
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Graph(#[from] StorageError),
    #[error(transparent)]
    Iri(#[from] IriParseError),
    #[error(transparent)]
    Serializer(#[from] SerializerError),
    #[error(transparent)]
    Evaluation(#[from] EvaluationError),
    #[error(transparent)]
    Serde(#[from] SerdeError),
    #[error(transparent)]
    FromUtf8(#[from] FromUtf8Error),
    #[error(transparent)]
    Loader(#[from] LoaderError),
    #[error(transparent)]
    RdfParse(#[from] RdfParseError),
}

#[derive(serde::Serialize)]
#[serde(tag = "kind", content = "message")]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    Graph(String),
    Iri(String),
    Serializer(String),
    Evaluation(String),
    Serde(String),
    FromUtf8(String),
    Loader(String),
    RdfParse(String),
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
      S: serde::ser::Serializer,
    {
      let error_message = self.to_string();
      let error_kind = match self {
        Self::Graph(_) => ErrorKind::Graph(error_message),
        Self::Iri(_) => ErrorKind::Iri(error_message),
        Self::Serializer(_) => ErrorKind::Serializer(error_message),
        Self::Evaluation(_) => ErrorKind::Evaluation(error_message),
        Self::Serde(_) => ErrorKind::Serde(error_message),
        Self::FromUtf8(_) => ErrorKind::FromUtf8(error_message),
        Self::Loader(_) => ErrorKind::Loader(error_message),
        Self::RdfParse(_) => ErrorKind::RdfParse(error_message),
      };
      error_kind.serialize(serializer)
    }
}

#[derive(Clone)]
pub struct Graph {
    store: Store,
}

impl Graph {
    pub fn open(db: &PathBuf) -> Result<Self, Error> {
        let store = Store::open(db)?;
        info!("Opened graph store at {:?}", db);
        Ok(Graph { store })
    }

    pub fn put_quad(
        &self,
        subject: &str,
        predicate: &str,
        object: &str,
        graph_name: Option<&str>,
    ) -> Result<Quad, Error> {
        let subject_node = NamedNodeRef::new(subject)?;
        let predicate_node = NamedNodeRef::new(predicate)?;
        let object_node = match object {
            // If the object is a URI (starts with http:// or https:// or ant://), create a NamedNodeRef
            _ if object.starts_with("http://") || object.starts_with("https://") || object.starts_with("ant://") => {
                TermRef::NamedNode(NamedNodeRef::new(object)?)
            },
            // Otherwise, treat it as a simple literal
            _ => TermRef::Literal(LiteralRef::new_simple_literal(object)),
        };
        let graph_name_ref = match graph_name {
            Some(name) => GraphNameRef::NamedNode(NamedNodeRef::new(name)?),
            None => GraphNameRef::DefaultGraph,
        };
        let quad = QuadRef::new(
            subject_node,
            predicate_node,
            object_node,
            graph_name_ref,
        );
        debug!("Creating quad: {:?}", quad);
        self.store.insert(quad)?;
        Ok(quad.into_owned())
    }

    pub fn add_pod_entry(&mut self, pod_name: &str, pod_address: &str, scratchpad_address: &str) -> Result<String, Error> {
        // Add a new pod
        let pod_iri = format!("ant://{}", pod_address);
        let pod_iri = pod_iri.as_str();
        let pod = NamedNodeRef::new(pod_iri)?;
        self.store.insert_named_graph(pod)?;

        // Enter in scratchpad quad
        let scratchpad_iri = format!("ant://{}", scratchpad_address);
        let scratchpad_iri = scratchpad_iri.as_str();
        let date = Utc::now().to_rfc3339();
        let date = date.as_str();
        // Pod metadata
        let _quad = self.put_quad(pod_iri,HAS_ADDR_TYPE,POD,None)?;
        let _quad = self.put_quad(pod_iri,HAS_NAME,pod_name,None)?;
        let _quad = self.put_quad(pod_iri,HAS_DEPTH,"0",None)?;
        let _quad = self.put_quad(pod_iri,HAS_DATE,date,Some(pod_iri))?;
        // Scratchpad metadata
        let _quad = self.put_quad(scratchpad_iri,HAS_POD_INDEX, "0", Some(pod_iri))?;
        debug!("Pod entries added");

        // Dump newly created graph in TriG format
        let mut buffer = Vec::new();
        self.store.dump_graph_to_writer(pod, RdfFormat::TriG, &mut buffer)?;

        Ok(buffer.into_iter().map(|b| b as char).collect())
    }

    pub fn pod_ref_entry(&mut self, pod_address: &str, pod_ref_address: &str, add: bool) -> Result<Vec<u8>, Error> {
        let pod_ref_iri = format!("ant://{}", pod_ref_address);
        let pod_ref_iri = pod_ref_iri.as_str();
        let pod_iri = format!("ant://{}", pod_address);
        let pod_iri = pod_iri.as_str();

        // Remove the depth object if it already exists in the default graph
        let update = format!(
            "DELETE WHERE {{ <{}> <{}> ?o . }}",
            pod_ref_iri, HAS_DEPTH
        );
        debug!("Delete pod_ref from default graph string: {}", update);
        self.store.update(update.as_str())?;

        // Delete existing data for the subject in the pod graph
        // This query deletes all triples for the subject in the specified pod graph
        let update = format!(
            "DELETE WHERE {{ GRAPH <{}> {{ <{}> ?p ?o . }} }}",
            pod_iri, pod_ref_iri
        );
        debug!("Delete pod_ref from pod string: {}", update);
  
        self.store.update(update.as_str())?;
  
        if add {
            // Enter in pod ref quad
            let _quad = self.put_quad(pod_ref_iri,HAS_DEPTH,"1",None)?;
            let _quad = self.put_quad(pod_ref_iri, HAS_ADDR_TYPE, POD_REF, Some(pod_iri))?;
            debug!("Pod ref {} added to pod {}", pod_ref_address, pod_address);
        } else {
            debug!("Pod ref {} removed from pod {}", pod_ref_address, pod_address);
        }

        // Dump the updated graph in TriG format
        let pod = oxigraph::model::NamedNodeRef::new(pod_iri)?;
        let mut buffer = Vec::new();
        self.store.dump_graph_to_writer(pod, RdfFormat::TriG, &mut buffer)?;

        Ok(buffer)
    }
        
    // Input is a JSON-LD string
    pub fn put_subject_data(&mut self, pod_address: &str, subject_address: &str, data: &str) -> Result<Vec<u8>, Error> {
        let pod_iri = format!("ant://{}", pod_address);
        let pod_iri = pod_iri.as_str();
        let pod = NamedNodeRef::new(pod_iri)?;
        let subject_iri = format!("ant://{}", subject_address);
        let subject_iri = subject_iri.as_str();

        // Delete existing data for the subject in the pod graph
        // This query deletes all triples for the subject in the specified pod graph
        let update = format!(
          "DELETE WHERE {{ GRAPH <{}> {{ <{}> ?p ?o . }} }}",
          pod_iri, subject_iri
        );
        debug!("Delete string: {}", update);

        self.store.update(update.as_str())?;

        // Convert the data &str to a Reader
        let data_reader = Cursor::new(data);

        // Insert the new data using the Reader
        //FIXME: may need the streaming profile option here?
        let mut profile = JsonLdProfileSet::empty();
        profile |= JsonLdProfile::Compacted;
        profile |= JsonLdProfile::Context;
        // Load the data into the pod graph
        self.store.load_from_reader(
            RdfParser::from_format(RdfFormat::JsonLd {
                profile: profile,
            })
            //.with_base_iri("https://schema.org/")? // don't need this
            .without_named_graphs() // No named graphs allowed in the input
            .with_default_graph(pod), // we put the file default graph inside of a named graph
            data_reader,
        )?;

        // Dump newly created graph in TriG format
        let mut buffer = Vec::new();
        self.store.dump_graph_to_writer(pod, RdfFormat::TriG, &mut buffer)?;

        Ok(buffer)
    }

    pub fn get_subject_data(&self, subject_address: &str) -> Result<String, Error> {
        let subject_iri = format!("ant://{}", subject_address);

        let query = format!(
            "SELECT ?p ?o WHERE {{ GRAPH ?g {{ <{}> ?p ?o . }} }}",
            subject_iri.as_str()
        );
        debug!("Query string: {}", query);

        let results = self.store.query(query.as_str())?;
        let buffer = results.write(Vec::new(), QueryResultsFormat::Json)?;

        // Map the vector buffer to a Value JSON object
        let json_str = String::from_utf8(buffer)?;
        debug!("Query results: {}", json_str);
        // This is output in the W3C JSON SPARQL format.
        // Can use the JavaScript `sparqljson-parse` library to parse it
        Ok(json_str)
    }

    // Get the depth of a pod from the graph database
    pub fn get_pod_depth(&self, pod_address: &str) -> Result<u64, Error> {
        let pod_iri = format!("ant://{}", pod_address);

        let query = format!(
            "SELECT ?depth WHERE {{ <{}> <{}> ?depth . }}",
            pod_iri, HAS_DEPTH
        );
        debug!("Depth query: {}", query);

        let results = self.store.query(query.as_str())?;
        if let QueryResults::Solutions(solutions) = results {
            for solution in solutions {
                if let Ok(solution) = solution {
                    if let Some(depth_term) = solution.get("depth") {
                        if let oxigraph::model::Term::Literal(literal) = depth_term {
                            if let Ok(depth_value) = literal.value().parse::<u64>() {
                                debug!("Found depth {} for pod {}", depth_value, pod_address);
                                return Ok(depth_value);
                            }
                        }
                    }
                }
            }
        }

        // If no depth found, return a high value to indicate unknown depth
        debug!("No depth found for pod {}, returning default", pod_address);
        Ok(u64::MAX)
    }

    // Update or set the depth of a pod in the graph database
    pub fn update_pod_depth(&mut self, pod_address: &str, new_depth: u64) -> Result<(), Error> {
        let pod_iri = format!("ant://{}", pod_address);

        // First, check if there's an existing depth
        let current_depth = self.get_pod_depth(pod_address)?;

        // Only update if the new depth is smaller (closer to root) or if no depth exists
        if new_depth < current_depth {
            info!("Updating depth for pod {} from {} to {}", pod_address, current_depth, new_depth);

            let delete_query = format!(
                "DELETE WHERE {{ <{}> <{}> ?depth . }}",
                pod_iri, HAS_DEPTH
            );
            debug!("Delete depth query: {}", delete_query);
            self.store.update(delete_query.as_str())?;

            // Insert new depth
            let _quad = self.put_quad(&pod_iri, HAS_DEPTH, &new_depth.to_string(), None)?;
            info!("Set depth {} for pod {}", new_depth, pod_address);
        } else {
            debug!("Not updating depth for pod {} (current: {}, new: {})", pod_address, current_depth, new_depth);
        }

        Ok(())
    }

    // Get all pods at a specific depth
    pub fn get_pods_at_depth(&self, depth: u64) -> Result<Vec<String>, Error> {
        let query = format!(
            "SELECT ?pod WHERE {{ ?pod <{}> \"{}\" . }}",
            HAS_DEPTH, depth
        );
        debug!("Pods at depth query: {}", query);

        let mut pods = Vec::new();
        let results = self.store.query(query.as_str())?;
        if let QueryResults::Solutions(solutions) = results {
            for solution in solutions {
                if let Ok(solution) = solution {
                    if let Some(pod_term) = solution.get("pod") {
                        if let oxigraph::model::Term::NamedNode(pod_node) = pod_term {
                            let pod_iri = pod_node.as_str();
                            // Extract the address from the ant:// URI
                            if let Some(address) = pod_iri.strip_prefix("ant://") {
                                pods.push(address.to_string());
                            }
                        }
                    }
                }
            }
        }

        debug!("Found {} pods at depth {}", pods.len(), depth);
        Ok(pods)
    }

    // Get all pod references from the graph data
    pub fn get_pod_references(&self, pod_address: &str) -> Result<Vec<String>, Error> {
        let pod_iri = format!("ant://{}", pod_address);

        // Query for all objects in the pod's named graph that are ant:// URIs
        let query = format!(
            "SELECT DISTINCT ?pod_ref WHERE {{ GRAPH <{}> {{ ?pod_ref <{}> <{}> . }} }}",
            pod_iri, HAS_ADDR_TYPE, POD_REF
        );
        debug!("Pod references query: {}", query);

        let mut references = Vec::new();
        let results = self.store.query(query.as_str())?;
        if let QueryResults::Solutions(solutions) = results {
            for solution in solutions {
                if let Ok(solution) = solution {
                    if let Some(ref_term) = solution.get("pod_ref") {
                        if let oxigraph::model::Term::NamedNode(ref_node) = ref_term {
                            let ref_iri = ref_node.as_str();
                            // Extract the address from the ant:// URI
                            if let Some(address) = ref_iri.strip_prefix("ant://") {
                                references.push(address.to_string());
                            }
                        }
                    }
                }
            }
        }

        debug!("Found {} references in pod {}", references.len(), pod_address);
        Ok(references)
    }

    // Load TriG data into the graph database
    pub fn load_pod_into_graph(&mut self, pod_address: &str,trig_data: &str) -> Result<(), Error> {
        if !trig_data.trim().is_empty() {
            let pod_iri = format!("ant://{}", pod_address);
            let pod_iri = pod_iri.as_str();
            let pod = NamedNodeRef::new(pod_iri)?;

            // Insert the graph if it wasn't already there
            self.store.insert_named_graph(pod)?;

            // Clear graph to receive new data
            self.store.clear_graph(pod)?;
    
            let data_reader = Cursor::new(trig_data);

            // Load the TriG data into the graph store
            self.store.load_from_reader(
                RdfParser::from_format(RdfFormat::TriG)
                .without_named_graphs() // No named graphs allowed in the input
                .with_default_graph(pod), // we put the file default graph inside of a named graph
                data_reader,
            )?;

            debug!("Successfully loaded TriG data into graph database");
        }

        Ok(())
    }

    // Search for content across all graphs
    pub fn search_content(&self, search_text: &str, limit: Option<u64>) -> Result<String, Error> {
        let limit_clause = match limit {
            Some(l) => format!("LIMIT {}", l),
            None => "".to_string(),
        };

        // Search for literal values containing the search text (case-insensitive)
        //FIXME: order the results by pod depth
        let query = format!(
            r#"
            SELECT DISTINCT ?subject ?predicate ?object ?graph WHERE {{
                GRAPH ?graph {{
                    ?subject ?predicate ?object .
                    FILTER(isLiteral(?object) && CONTAINS(LCASE(STR(?object)), LCASE("{}")))
                }}
            }}
            ORDER BY ?graph ?subject
            {}
            "#,
            search_text.replace("\"", "\\\""), // Escape quotes in search text
            limit_clause
        );

        debug!("Search query: {}", query);

        let results = self.store.query(query.as_str())?;
        let buffer = results.write(Vec::new(), QueryResultsFormat::Json)?;
        let json_str = String::from_utf8(buffer)?;

        debug!("Search results: {}", json_str);
        Ok(json_str)
    }

    // Search for subjects by type
    //FIXME: order the results by pod depth
    pub fn search_by_type(&self, type_uri: &str, limit: Option<u64>) -> Result<String, Error> {
        let limit_clause = match limit {
            Some(l) => format!("LIMIT {}", l),
            None => "".to_string(),
        };

        let query = format!(
            r#"
            SELECT DISTINCT ?subject ?graph WHERE {{
                GRAPH ?graph {{
                    ?subject <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <{}> .
                }}
            }}
            ORDER BY ?graph ?subject
            {}
            "#,
            type_uri,
            limit_clause
        );

        debug!("Type search query: {}", query);

        let results = self.store.query(query.as_str())?;
        let buffer = results.write(Vec::new(), QueryResultsFormat::Json)?;
        let json_str = String::from_utf8(buffer)?;

        debug!("Type search results: {}", json_str);
        Ok(json_str)
    }

    // Search for subjects with a specific predicate
    pub fn search_by_predicate(&self, predicate_uri: &str, limit: Option<u64>) -> Result<String, Error> {
        let limit_clause = match limit {
            Some(l) => format!("LIMIT {}", l),
            None => "".to_string(),
        };

        let query = format!(
            r#"
            SELECT DISTINCT ?subject ?object ?graph WHERE {{
                GRAPH ?graph {{
                    ?subject <{}> ?object .
                }}
            }}
            ORDER BY ?graph ?subject
            {}
            "#,
            predicate_uri,
            limit_clause
        );

        debug!("Predicate search query: {}", query);

        let results = self.store.query(query.as_str())?;
        let buffer = results.write(Vec::new(), QueryResultsFormat::Json)?;
        let json_str = String::from_utf8(buffer)?;

        debug!("Predicate search results: {}", json_str);
        Ok(json_str)
    }

    // Advanced search with multiple criteria
    pub fn advanced_search(&self, criteria: &serde_json::Value) -> Result<String, Error> {
        // Build SPARQL query based on criteria
        let mut where_clauses = Vec::new();
        let mut filters = Vec::new();

        // Handle text search
        if let Some(text) = criteria.get("text").and_then(|v| v.as_str()) {
            if !text.is_empty() {
                where_clauses.push("?subject ?predicate ?object .".to_string());
                filters.push(format!(
                    "FILTER(isLiteral(?object) && CONTAINS(LCASE(STR(?object)), LCASE(\"{}\")))",
                    text.replace("\"", "\\\"")
                ));
            }
        }

        // Handle type filter
        if let Some(type_uri) = criteria.get("type").and_then(|v| v.as_str()) {
            if !type_uri.is_empty() {
                where_clauses.push(format!(
                    "?subject <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <{}> .",
                    type_uri
                ));
            }
        }

        // Handle predicate filter
        if let Some(predicate) = criteria.get("predicate").and_then(|v| v.as_str()) {
            if !predicate.is_empty() {
                where_clauses.push(format!("?subject <{}> ?object .", predicate));
            }
        }

        // Handle pod filter (specific graph)
        if let Some(pod_address) = criteria.get("pod").and_then(|v| v.as_str()) {
            if !pod_address.is_empty() {
                let _pod_iri = if pod_address.starts_with("ant://") {
                    pod_address.to_string()
                } else {
                    format!("ant://{}", pod_address)
                };
                // This will be used in the GRAPH clause
            }
        }

        // Default to basic search if no criteria
        if where_clauses.is_empty() {
            where_clauses.push("?subject ?predicate ?object .".to_string());
        }

        let limit = criteria.get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(100);

        let where_clause = where_clauses.join(" ");
        let filter_clause = if filters.is_empty() {
            "".to_string()
        } else {
            filters.join(" ")
        };

        let query = format!(
            r#"
            SELECT DISTINCT ?subject ?predicate ?object ?graph WHERE {{
                GRAPH ?graph {{
                    {}
                    {}
                }}
            }}
            ORDER BY ?graph ?subject
            LIMIT {}
            "#,
            where_clause,
            filter_clause,
            limit
        );

        debug!("Advanced search query: {}", query);

        let results = self.store.query(query.as_str())?;
        let buffer = results.write(Vec::new(), QueryResultsFormat::Json)?;
        let json_str = String::from_utf8(buffer)?;

        debug!("Advanced search results: {}", json_str);
        Ok(json_str)
    }

    // Get all scratchpad addresses for a pod
    pub fn get_pod_scratchpads(&self, pod_address: &str) -> Result<Vec<String>, Error> {
        let pod_iri = format!("ant://{}", pod_address);

        // Query for all scratchpad addresses in the pod's named graph
        let query = format!(
            "SELECT DISTINCT ?scratchpad ?index WHERE {{ GRAPH <{}> {{ ?scratchpad <{}> ?index . }} }}",
            pod_iri, HAS_POD_INDEX
        );
        debug!("Pod scratchpads query: {}", query);

        let mut triples = HashMap::new();
        let results = self.store.query(query.as_str())?;
        if let QueryResults::Solutions(solutions) = results {
            for solution in solutions {
                if let Ok(solution) = solution {
                    if let Some(scratchpad_term) = solution.get("scratchpad") {
                        if let oxigraph::model::Term::NamedNode(scratchpad_node) = scratchpad_term {
                            let scratchpad_iri = scratchpad_node.as_str();
                            // Extract the address from the ant:// URI
                            if let Some(address) = scratchpad_iri.strip_prefix("ant://") {
                                if let Some(index_term) = solution.get("index") {
                                    if let oxigraph::model::Term::Literal(literal) = index_term {
                                        if let Ok(index) = literal.value().parse::<u64>() {
                                            triples.insert(index, address.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // take the ordered addresses from the hashmap and map them to the scratchpads vector
        let mut scratchpads = Vec::new();
        for i in 0..triples.len() {
            if let Some(scratchpad) = triples.get(&(i as u64)) {
                // The address is already stripped of "ant://" prefix in line 661
                scratchpads.push(scratchpad.clone());
            } else {
                error!("Missing scratchpad at index {}", i);
            }
        }

        debug!("Found {} scratchpads for pod {}", scratchpads.len(), pod_address);
        Ok(scratchpads)
    }

    pub fn get_pod_scratchpads_from_string(&self, data: &str) -> Result<Vec<String>, Error> {
        
        // Parse the TriG data and return a hashmap of the scratchpad addresses and their pod index
        let mut triples = HashMap::new();
        for triple in TriGParser::new().for_reader(data.as_bytes()) {
            // The last line will be garbage, so we just ignore it by passing a default quad
            let triple = triple.unwrap_or_else(
                |_e| Quad::new(
                                            NamedNodeRef::new("http://example.org/subject").unwrap(),
                                            NamedNodeRef::new("http://example.org/predicate").unwrap(), 
                                            NamedNodeRef::new("http://example.org/object").unwrap(), 
                                            GraphNameRef::DefaultGraph));
            
            if triple.predicate == HAS_POD_INDEX {
                // Convert the triple.object into a u64
                if let oxigraph::model::Term::Literal(literal) = triple.object {
                    if let Ok(index) = literal.value().parse::<u64>() {
                        if let oxigraph::model::Subject::NamedNode(scratchpad) = triple.subject {
                            triples.insert(index, scratchpad.into_string());
                        }
                    }
                }
            }
        }

        // take the ordered addresses from the hashmap and map them to the scratchpads vector
        let mut scratchpads = Vec::new();
        for i in 0..triples.len() {
            if let Some(scratchpad) = triples.get(&(i as u64)) {
                let address = scratchpad.as_str().strip_prefix("ant://").unwrap_or_default();
                scratchpads.push(address.to_string());
            } else {
                error!("Missing scratchpad at index {}", i);
            }
        }

        Ok(scratchpads)
    }

    // Clear a specific pod graph
    pub fn clear_pod_graph(&mut self, pod_address: &str) -> Result<(), Error> {
        let pod_iri = format!("ant://{}", pod_address);
        let pod_node = NamedNodeRef::new(&pod_iri)?;
        self.store.clear_graph(pod_node)?;
        debug!("Cleared graph for pod: {}", pod_address);
        Ok(())
    }

}



