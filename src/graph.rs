use oxigraph::io::{RdfFormat, RdfParser, RdfParseError};
use tracing::{info, debug, error};
use thiserror;
use serde;
use oxigraph::sparql::EvaluationError;
use oxigraph::model::{NamedNodeRef, IriParseError, QuadRef, TermRef, GraphNameRef, LiteralRef, Quad};
use oxigraph::store::{SerializerError, StorageError, Store, LoaderError};
use oxigraph::sparql::results::QueryResultsFormat;
use std::path::PathBuf;
use serde_json::{Error as SerdeError};
use alloc::string::FromUtf8Error;
use chrono::Utc;
use oxjsonld::{JsonLdProfile, JsonLdProfileSet};
use std::io::Cursor;

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
#[allow(dead_code)]
const POD: &str = OBJECT!("pod");
#[allow(dead_code)]
const POD_REF: &str = OBJECT!("pod_ref");
const POD_SCRATCHPAD: &str = OBJECT!("scratchpad");

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

    pub fn add_pod_entry(&mut self, pod_address: &str, scratchpad_address: &str) -> Result<String, Error> {
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
        let _quad = self.put_quad(scratchpad_iri,HAS_ADDR_TYPE,POD_SCRATCHPAD,Some(pod_iri))?;
        let _quad = self.put_quad(scratchpad_iri,HAS_NAME,"Unnamed Pod",None)?;
        let _quad = self.put_quad(pod_iri,HAS_DEPTH,"0",None)?; // Set depth on pod IRI, not scratchpad
        let _quad = self.put_quad(scratchpad_iri,HAS_POD_INDEX, "0", Some(pod_iri))?;
        let _quad = self.put_quad(scratchpad_iri,HAS_DATE,date,Some(pod_iri))?;
        debug!("Pod entries added");

        // Dump newly created graph in TriG format
        let mut buffer = Vec::new();
        self.store.dump_graph_to_writer(pod, RdfFormat::TriG, &mut buffer)?;

        Ok(buffer.into_iter().map(|b| b as char).collect())
    }

    // pub fn get_pod_scratchpads(&self, pointer_address: &str) -> Result<Vec<String>, Error> {
    //     let pointer_iri = format!("ant://{}", pointer_address);
    //     let pod = NamedNodeRef::new(pointer_iri.as_str())?;
    //     let query = format!(
    //         "SELECT ?scratchpad WHERE {{ GRAPH <{}> {{ ?scratchpad <{}> <{}> . }} }}",
    //         pod, HAS_ADDR_TYPE, POD
    //     );

    //     let mut scratchpads = Vec::new();
    //     if let QueryResults::Solutions(solutions) = self.store.query(&query)? {
    //         for solution in solutions {
    //             if let Some(scratchpad) = solution.get("scratchpad") {
    //                 if let TermRef::NamedNode(scratchpad_node) = scratchpad {
    //                     scratchpads.push(scratchpad_node.to_string());
    //                 }
    //             }
    //         }
    //     } else {
    //         error!("Query did not return solutions");
    //     }
    //     debug!("Found {} scratchpads for pod {}", scratchpads.len(), pointer_address);
    //     Ok(scratchpads)
    // }

    // Input is a JSON-LD string
    pub fn put_subject_data(&mut self, pod_address: &str, subject_address: &str, data: &str) -> Result<Vec<u8>, Error> {
        let pod_iri = format!("ant://{}", pod_address);
        let pod_iri = pod_iri.as_str();
        let pod = NamedNodeRef::new(pod_iri)?;
        let subject_iri = format!("ant://{}", subject_address);
        let subject_iri = subject_iri.as_str();
        let _subject = NamedNodeRef::new(subject_iri)?;

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
        if let oxigraph::sparql::QueryResults::Solutions(solutions) = results {
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

            // Delete existing depth if it exists
            if current_depth != u64::MAX {
                let delete_query = format!(
                    "DELETE WHERE {{ <{}> <{}> ?depth . }}",
                    pod_iri, HAS_DEPTH
                );
                debug!("Delete depth query: {}", delete_query);
                self.store.update(delete_query.as_str())?;
            }

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
        if let oxigraph::sparql::QueryResults::Solutions(solutions) = results {
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
            "SELECT DISTINCT ?ref WHERE {{ GRAPH <{}> {{ ?s ?p ?ref . FILTER(isIRI(?ref) && STRSTARTS(STR(?ref), \"ant://\")) }} }}",
            pod_iri
        );
        debug!("Pod references query: {}", query);

        let mut references = Vec::new();
        let results = self.store.query(query.as_str())?;
        if let oxigraph::sparql::QueryResults::Solutions(solutions) = results {
            for solution in solutions {
                if let Ok(solution) = solution {
                    if let Some(ref_term) = solution.get("ref") {
                        if let oxigraph::model::Term::NamedNode(ref_node) = ref_term {
                            let ref_iri = ref_node.as_str();
                            // Only include URIs that don't contain vocabulary (to exclude predicate/object URIs)
                            if !ref_iri.contains("/vocabulary/") && ref_iri != pod_iri {
                                references.push(ref_iri.to_string());
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
    pub fn load_trig_data(&mut self, trig_data: &str) -> Result<(), Error> {
        if !trig_data.trim().is_empty() {
            let data_reader = Cursor::new(trig_data);

            // Load the TriG data into the graph store
            self.store.load_from_reader(
                RdfParser::from_format(RdfFormat::TriG),
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

}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_graph() -> (Graph, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test_graph.db");
        let graph = Graph::open(&db_path).expect("Failed to create test graph");
        (graph, temp_dir)
    }

    #[test]
    fn test_graph_creation() {
        let (_graph, _temp_dir) = create_test_graph();
        // Graph should be created successfully
        assert!(true); // If we get here, graph creation worked
    }

    #[test]
    fn test_add_pod_entry() {
        let (mut graph, _temp_dir) = create_test_graph();

        let pod_address = "1234567890abcdef";
        let scratchpad_address = "abcdef1234567890";

        let result = graph.add_pod_entry(pod_address, scratchpad_address);
        assert!(result.is_ok());

        let trig_data = result.unwrap();
        assert!(!trig_data.is_empty());
        // The function creates a named graph for the pod and adds data about the scratchpad
        assert!(trig_data.contains(&format!("ant://{}", scratchpad_address)));
        // Check for the actual predicate URIs
        assert!(trig_data.contains("colonylib/vocabulary"));
        // Note: depth is stored in the default graph, not in the pod's named graph
        // so it won't appear in the TriG output for the specific pod graph
    }

    #[test]
    fn test_pod_depth_operations() {
        let (mut graph, _temp_dir) = create_test_graph();

        let pod_address = "test_pod_123";

        // Initially, pod should have no depth (returns u64::MAX)
        let initial_depth = graph.get_pod_depth(pod_address).unwrap();
        assert_eq!(initial_depth, u64::MAX);

        // Set depth to 0
        graph.update_pod_depth(pod_address, 0).unwrap();
        let depth = graph.get_pod_depth(pod_address).unwrap();
        assert_eq!(depth, 0);

        // Try to set depth to 2 (should NOT work since 2 > 0, depth should remain 0)
        graph.update_pod_depth(pod_address, 2).unwrap();
        let depth = graph.get_pod_depth(pod_address).unwrap();
        assert_eq!(depth, 0); // Should still be 0 since we only update to smaller depths

        // Try to set depth to 1 (should NOT work since 1 > 0, depth should remain 0)
        graph.update_pod_depth(pod_address, 1).unwrap();
        let depth = graph.get_pod_depth(pod_address).unwrap();
        assert_eq!(depth, 0); // Should still be 0

        // Now let's test with a higher initial depth
        // First set depth to 5
        graph.update_pod_depth(pod_address, 5).unwrap(); // This won't work since 5 > 0
        let depth = graph.get_pod_depth(pod_address).unwrap();
        assert_eq!(depth, 0); // Should still be 0

        // Let's start fresh with a new pod to test the depth logic properly
        let pod_address2 = "test_pod_456";

        // Set initial depth to 5 (this should work since no depth exists)
        graph.update_pod_depth(pod_address2, 5).unwrap();
        let depth = graph.get_pod_depth(pod_address2).unwrap();
        assert_eq!(depth, 5);

        // Try to set depth to 3 (should work since 3 < 5)
        graph.update_pod_depth(pod_address2, 3).unwrap();
        let depth = graph.get_pod_depth(pod_address2).unwrap();
        assert_eq!(depth, 3);

        // Try to set depth to 7 (should not change since 7 > 3)
        graph.update_pod_depth(pod_address2, 7).unwrap();
        let depth = graph.get_pod_depth(pod_address2).unwrap();
        assert_eq!(depth, 3); // Should still be 3
    }

    #[test]
    fn test_get_pods_at_depth() {
        let (mut graph, _temp_dir) = create_test_graph();

        let pod1 = "pod1_address";
        let pod2 = "pod2_address";
        let pod3 = "pod3_address";

        // Set different depths
        graph.update_pod_depth(pod1, 0).unwrap();
        graph.update_pod_depth(pod2, 1).unwrap();
        graph.update_pod_depth(pod3, 0).unwrap();

        // Get pods at depth 0
        let pods_at_depth_0 = graph.get_pods_at_depth(0).unwrap();
        assert_eq!(pods_at_depth_0.len(), 2);
        assert!(pods_at_depth_0.contains(&pod1.to_string()));
        assert!(pods_at_depth_0.contains(&pod3.to_string()));

        // Get pods at depth 1
        let pods_at_depth_1 = graph.get_pods_at_depth(1).unwrap();
        assert_eq!(pods_at_depth_1.len(), 1);
        assert!(pods_at_depth_1.contains(&pod2.to_string()));

        // Get pods at depth 2 (should be empty)
        let pods_at_depth_2 = graph.get_pods_at_depth(2).unwrap();
        assert_eq!(pods_at_depth_2.len(), 0);
    }

    #[test]
    fn test_load_trig_data() {
        let (mut graph, _temp_dir) = create_test_graph();

        // Test with empty data
        let result = graph.load_trig_data("");
        assert!(result.is_ok());

        // Test with whitespace only
        let result = graph.load_trig_data("   \n\t  ");
        assert!(result.is_ok());

        // Test with simple TriG data
        let trig_data = r#"
            @prefix ex: <http://example.org/> .
            ex:graph1 {
                ex:subject ex:predicate ex:object .
            }
        "#;

        let result = graph.load_trig_data(trig_data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_pod_references() {
        let (mut graph, _temp_dir) = create_test_graph();

        let pod_address = "test_pod";

        // Create a pod with some test data that includes references
        let trig_data = format!(r#"
            @prefix ant: <ant://> .
            <ant://{}> {{
                <ant://subject1> <ant://colonylib/vocabulary/0.1/predicate#references> <ant://referenced_pod1> .
                <ant://subject2> <ant://colonylib/vocabulary/0.1/predicate#references> <ant://referenced_pod2> .
                <ant://subject3> <ant://colonylib/vocabulary/0.1/predicate#name> "Some Name" .
            }}
        "#, pod_address);

        // Load the test data
        graph.load_trig_data(&trig_data).unwrap();

        // Get references
        let references = graph.get_pod_references(pod_address).unwrap();

        // Should find the referenced pods but not vocabulary URIs
        assert!(references.contains(&"ant://referenced_pod1".to_string()));
        assert!(references.contains(&"ant://referenced_pod2".to_string()));

        // Should not contain vocabulary URIs or the pod itself
        assert!(!references.iter().any(|r| r.contains("/vocabulary/")));
        assert!(!references.contains(&format!("ant://{}", pod_address)));
    }

    #[test]
    fn test_put_quad() {
        let (graph, _temp_dir) = create_test_graph();

        let subject = "ant://test_subject";
        let predicate = "ant://colonylib/vocabulary/0.1/predicate#test";
        let object = "test_value";

        let result = graph.put_quad(subject, predicate, object, None);
        assert!(result.is_ok());

        // Test with named graph
        let graph_name = "ant://test_graph";
        let result = graph.put_quad(subject, predicate, object, Some(graph_name));
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_content() {
        let (graph, _temp_dir) = create_test_graph();

        // Add some test data
        let pod_address = "test_pod";
        let pod_iri = format!("ant://{}", pod_address);

        // Add test triples with searchable content
        graph.put_quad(
            "ant://subject1",
            "ant://colonylib/vocabulary/0.1/predicate#name",
            "Test Document",
            Some(&pod_iri)
        ).unwrap();

        graph.put_quad(
            "ant://subject2",
            "ant://colonylib/vocabulary/0.1/predicate#description",
            "This is a test description with searchable content",
            Some(&pod_iri)
        ).unwrap();

        graph.put_quad(
            "ant://subject3",
            "ant://colonylib/vocabulary/0.1/predicate#name",
            "Another Document",
            Some(&pod_iri)
        ).unwrap();

        // Test text search
        let results = graph.search_content("test", Some(10)).unwrap();
        let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();

        // Should find results containing "test"
        assert!(parsed_results.get("results").is_some());
        let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
        assert!(bindings.len() > 0);

        // Test case-insensitive search
        let results = graph.search_content("TEST", Some(10)).unwrap();
        let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();
        let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
        assert!(bindings.len() > 0);

        // Test search with no results
        let results = graph.search_content("nonexistent", Some(10)).unwrap();
        let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();
        let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
        assert_eq!(bindings.len(), 0);
    }

    #[test]
    fn test_search_by_type() {
        let (graph, _temp_dir) = create_test_graph();

        let pod_address = "test_pod";
        let pod_iri = format!("ant://{}", pod_address);

        // Add test data with types
        graph.put_quad(
            "ant://subject1",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            "http://schema.org/MediaObject",
            Some(&pod_iri)
        ).unwrap();

        graph.put_quad(
            "ant://subject2",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            "http://schema.org/Person",
            Some(&pod_iri)
        ).unwrap();

        // Test type search
        let results = graph.search_by_type("http://schema.org/MediaObject", Some(10)).unwrap();
        let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();

        let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
        assert_eq!(bindings.len(), 1);

        // Verify the correct subject was found
        let subject_value = bindings[0]["subject"]["value"].as_str().unwrap();
        assert_eq!(subject_value, "ant://subject1");
    }

    #[test]
    fn test_search_by_predicate() {
        let (graph, _temp_dir) = create_test_graph();

        let pod_address = "test_pod";
        let pod_iri = format!("ant://{}", pod_address);

        // Add test data with specific predicates
        graph.put_quad(
            "ant://subject1",
            "ant://colonylib/vocabulary/0.1/predicate#name",
            "Test Name",
            Some(&pod_iri)
        ).unwrap();

        graph.put_quad(
            "ant://subject2",
            "ant://colonylib/vocabulary/0.1/predicate#description",
            "Test Description",
            Some(&pod_iri)
        ).unwrap();

        // Test predicate search
        let results = graph.search_by_predicate(
            "ant://colonylib/vocabulary/0.1/predicate#name",
            Some(10)
        ).unwrap();
        let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();

        let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
        assert_eq!(bindings.len(), 1);

        // Verify the correct subject and object were found
        let subject_value = bindings[0]["subject"]["value"].as_str().unwrap();
        let object_value = bindings[0]["object"]["value"].as_str().unwrap();
        assert_eq!(subject_value, "ant://subject1");
        assert_eq!(object_value, "Test Name");
    }

    #[test]
    fn test_advanced_search() {
        let (graph, _temp_dir) = create_test_graph();

        let pod_address = "test_pod";
        let pod_iri = format!("ant://{}", pod_address);

        // Add comprehensive test data
        graph.put_quad(
            "ant://subject1",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            "http://schema.org/MediaObject",
            Some(&pod_iri)
        ).unwrap();

        graph.put_quad(
            "ant://subject1",
            "ant://colonylib/vocabulary/0.1/predicate#name",
            "Test Media File",
            Some(&pod_iri)
        ).unwrap();

        // Test advanced search with text criteria
        let criteria = serde_json::json!({
            "text": "media",
            "limit": 10
        });

        let results = graph.advanced_search(&criteria).unwrap();
        let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();

        let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
        assert!(bindings.len() > 0);

        // Test advanced search with type criteria
        let criteria = serde_json::json!({
            "type": "http://schema.org/MediaObject",
            "limit": 10
        });

        let results = graph.advanced_search(&criteria).unwrap();
        let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();

        let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
        assert_eq!(bindings.len(), 1);

        // Test advanced search with predicate criteria
        let criteria = serde_json::json!({
            "predicate": "ant://colonylib/vocabulary/0.1/predicate#name",
            "limit": 10
        });

        let results = graph.advanced_search(&criteria).unwrap();
        let parsed_results: serde_json::Value = serde_json::from_str(&results).unwrap();

        let bindings = parsed_results["results"]["bindings"].as_array().unwrap();
        assert_eq!(bindings.len(), 1);
    }
}