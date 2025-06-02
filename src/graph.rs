use oxigraph::io::RdfFormat;
use tracing::{info, debug, error};
use thiserror;
use serde;
use oxigraph::sparql::EvaluationError;
use oxigraph::model::{NamedNodeRef, IriParseError, QuadRef, SubjectRef, TermRef, GraphNameRef, LiteralRef};
use oxigraph::store::{SerializerError, StorageError, Store};
use oxigraph::sparql::results::QueryResultsFormat;
use std::path::PathBuf;
use serde_json::{Value, Error as SerdeError};
use alloc::string::FromUtf8Error;

//////////////////////////////////////////////
// Vocabulary
//////////////////////////////////////////////
const VERSION: &str = "0.1"; // use this version for all pods made in this version of the library
macro_rules! PREDICATE {
  ($e:expr) => {
      concat!("colonylib://vocabulary/", "0.1/", "predicate#", $e)
  };
}

macro_rules! OBJECT {
  ($e:expr) => {
      concat!("colonylib://vocabulary/", "0.1/", "object#", $e)
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

/// Version
/// The version of vocabulary used in this pod
/// Must always be the VERSION constant
const HAS_VERSION: &str = PREDICATE!("version");

/// File Type
/// Defines the type of the file
/// Object must be one of the file type objects
const HAS_FILE_TYPE: &str = PREDICATE!("file_type");

/// File Size
/// The size of the file in bytes
/// Object is a literal representing the size
const HAS_FILE_SIZE: &str = PREDICATE!("file_size");

/// File Description
/// A description of the file
/// Object is a string literal
const HAS_FILE_DESCRIPTION: &str = PREDICATE!("file_description");

/// File Comment
/// A comment about the file
/// Object is a string literal
const HAS_FILE_COMMENT: &str = PREDICATE!("file_comment");

//////////////////////////////////////////////
// Objects
//////////////////////////////////////////////

/// Address Type Objects
/// Defines what kind of object the address is pointing to
const POD: &str = OBJECT!("pod");
const POD_REF: &str = OBJECT!("pod_ref");
const SCRATCHPAD: &str = OBJECT!("scratchpad");
const FILE: &str = OBJECT!("file");

/// File Type Objects
/// Defines what kind of file the address is pointing to
const MUSIC: &str = OBJECT!("music");
const IMAGE: &str = OBJECT!("image");
const VIDEO: &str = OBJECT!("video");
const TEXT: &str = OBJECT!("text");
const DOCUMENT: &str = OBJECT!("document");
const ARCHIVE: &str = OBJECT!("archive");
const BINARY: &str = OBJECT!("binary");

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

    pub fn add_pod_entry(&mut self, pointer_address: &str, scratchpad_address: &str) -> Result<String, Error> {
        // Add a new pod
        let pointer_iri = format!("ant://{}", pointer_address);
        let pod = NamedNodeRef::new(pointer_iri.as_str())?;
        self.store.insert_named_graph(pod)?;

        // Enter in scratchpad quad
        let scratchpad_iri = format!("ant://{}", scratchpad_address);
        let scratchpad = NamedNodeRef::new(scratchpad_iri.as_str())?;
        let scratchpad_type = NamedNodeRef::new(SCRATCHPAD)?;
        let quad = QuadRef::new(
            SubjectRef::NamedNode(scratchpad),
            NamedNodeRef::new(HAS_ADDR_TYPE)?,
            TermRef::NamedNode(scratchpad_type),
            GraphNameRef::NamedNode(pod),
        );
        debug!("Adding scratchpad entry: {:?}", quad);
        self.store.insert(quad)?;

        let version = LiteralRef::new_simple_literal(VERSION);
        let quad = QuadRef::new(
            SubjectRef::NamedNode(scratchpad),
            NamedNodeRef::new(HAS_VERSION)?,
            TermRef::Literal(version),
            GraphNameRef::NamedNode(pod),
        );
        debug!("Applied pod version attribute: {:?}", quad);
        self.store.insert(quad)?;

        let name = LiteralRef::new_simple_literal("example pod");
        let quad = QuadRef::new(
            SubjectRef::NamedNode(scratchpad),
            NamedNodeRef::new(HAS_NAME)?,
            TermRef::Literal(name),
            GraphNameRef::NamedNode(pod),
        );
        debug!("Applied pod name attribute: {:?}", quad);
        self.store.insert(quad)?;
        debug!("Pod entry added");

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

    pub fn put_subject_data(&mut self, pointer_address: &str, subject_address: &str, data: Value) -> Result<Vec<u8>, Error> {

        // Process the data and convert it to a format suitable for insertion
        let pointer_iri = format!("ant://{}", pointer_address);
        let pod = NamedNodeRef::new(pointer_iri.as_str())?;
        let subject_iri = format!("ant://{}", subject_address);
        let subject = NamedNodeRef::new(subject_iri.as_str())?;
        if let Some(name) = data.get("name") {
            let name_literal = LiteralRef::new_simple_literal(name.as_str().unwrap_or("Unnamed Pod"));
            let quad = QuadRef::new(
                SubjectRef::NamedNode(subject),
                NamedNodeRef::new(HAS_NAME)?,
                TermRef::Literal(name_literal),
                GraphNameRef::NamedNode(pod),
            );
            // check if the name is already set
            self.store.remove(quad)?;
            self.store.insert(quad)?;
        }
        if let Some(version) = data.get("version") {
            let version_literal = LiteralRef::new_simple_literal(version.as_str().unwrap_or(VERSION));
            let quad = QuadRef::new(
                SubjectRef::NamedNode(subject),
                NamedNodeRef::new(HAS_VERSION)?,
                TermRef::Literal(version_literal),
                GraphNameRef::NamedNode(pod),
            );
            self.store.remove(quad)?;
            self.store.insert(quad)?;
        }
        if let Some(addr_type) = data.get("addr_type") {
            let addr_type_iri = match addr_type.as_str() {
                Some("pod") => POD,
                Some("pod_ref") => POD_REF,
                Some("scratchpad") => SCRATCHPAD,
                Some("file") => FILE,
                _ => return Err(StorageError::Other(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Invalid address type"))).into()),
            };
            let addr_type_node = NamedNodeRef::new(addr_type_iri)?;
            let quad = QuadRef::new(
                SubjectRef::NamedNode(subject),
                NamedNodeRef::new(HAS_ADDR_TYPE)?,
                TermRef::NamedNode(addr_type_node),
                GraphNameRef::NamedNode(pod),
            );
            self.store.remove(quad)?;
            self.store.insert(quad)?;
        }
        if let Some(file_type) = data.get("file_type") {
            let file_type_iri = match file_type.as_str() {
                Some("music") => MUSIC,
                Some("image") => IMAGE,
                Some("video") => VIDEO,
                Some("text") => TEXT,
                Some("document") => DOCUMENT,
                Some("archive") => ARCHIVE,
                Some("binary") => BINARY,
                _ => return Err(StorageError::Other(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Invalid file type"))).into()),
            };
            let file_type_node = NamedNodeRef::new(file_type_iri)?;
            let quad = QuadRef::new(
                SubjectRef::NamedNode(subject),
                NamedNodeRef::new(HAS_FILE_TYPE)?,
                TermRef::NamedNode(file_type_node),
                GraphNameRef::NamedNode(pod),
            );
            self.store.remove(quad)?;
            self.store.insert(quad)?;
        }
        if let Some(file_size) = data.get("file_size") {
            let file_size_literal = LiteralRef::new_simple_literal(file_size.as_str().unwrap_or("0"));
            let quad = QuadRef::new(
                SubjectRef::NamedNode(subject),
                NamedNodeRef::new(HAS_FILE_SIZE)?,
                TermRef::Literal(file_size_literal),
                GraphNameRef::NamedNode(pod),
            );
            self.store.remove(quad)?;
            self.store.insert(quad)?;
        }
        if let Some(file_description) = data.get("file_description") {
            let file_description_literal = LiteralRef::new_simple_literal(file_description.as_str().unwrap_or(""));
            let quad = QuadRef::new(
                SubjectRef::NamedNode(subject),
                NamedNodeRef::new(HAS_FILE_DESCRIPTION)?,
                TermRef::Literal(file_description_literal),
                GraphNameRef::NamedNode(pod),
            );
            self.store.remove(quad)?;
            self.store.insert(quad)?;
        }
        if let Some(file_comment) = data.get("file_comment") {
            let file_comment_literal = LiteralRef::new_simple_literal(file_comment.as_str().unwrap_or(""));
            let quad = QuadRef::new(
                SubjectRef::NamedNode(subject),
                NamedNodeRef::new(HAS_FILE_COMMENT)?,
                TermRef::Literal(file_comment_literal),
                GraphNameRef::NamedNode(pod),
            );
            self.store.remove(quad)?;
            self.store.insert(quad)?;
        }

        // Dump newly created graph in TriG format
        let mut buffer = Vec::new();
        self.store.dump_graph_to_writer(pod, RdfFormat::TriG, &mut buffer)?;
        Ok(buffer)
    }

    pub fn get_subject_data(&self, subject_address: &str) -> Result<Value, Error> {
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
        //FIXME: this is output in the W3C JSON SPARQL format.
        // Do we want to convert this to a JSON format that matches the input to put_subject_data?
        let json_value: Value = serde_json::from_str(&json_str)?;

        Ok(json_value)
    }

}