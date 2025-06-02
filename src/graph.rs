use oxigraph::io::{RdfFormat, RdfParser, RdfParseError};
use tracing::{info, debug, error};
use thiserror;
use serde;
use oxigraph::sparql::EvaluationError;
use oxigraph::model::{NamedNodeRef, IriParseError, QuadRef, SubjectRef, TermRef, GraphNameRef, LiteralRef, Quad};
use oxigraph::store::{SerializerError, StorageError, Store, LoaderError};
use oxigraph::sparql::results::QueryResultsFormat;
use std::path::PathBuf;
use serde_json::{Value, Error as SerdeError};
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
const POD: &str = OBJECT!("pod");
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

    fn put_quad(
        &self,
        subject: &str,
        predicate: &str,
        object: &str,
        graph_name: Option<&str>,
    ) -> Result<Quad, Error> {
        let subject_node = NamedNodeRef::new(subject)?;
        let predicate_node = NamedNodeRef::new(predicate)?;
        let object_node = match object {
            // If the object is an address, create a NamedNodeRef
            _ if object.starts_with("ant://") => TermRef::NamedNode(NamedNodeRef::new(object)?),
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
        let _quad = self.put_quad(scratchpad_iri,HAS_DEPTH,"0",None)?;
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
        let subject = NamedNodeRef::new(subject_iri)?;

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

}