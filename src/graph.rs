use tracing::{debug, error, info, warn, instrument};
use thiserror;
use serde;
use oxigraph::sparql::QueryResults;
use oxigraph::store::{StorageError, Store};
use std::path::PathBuf;
use std::fmt;


// Error handling
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Graph(#[from] StorageError),
}

// Removed manual Display implementation to avoid conflict with thiserror::Error

#[derive(serde::Serialize)]
#[serde(tag = "kind", content = "message")]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    Graph(String),
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
      S: serde::ser::Serializer,
    {
      let error_message = self.to_string();
      let error_kind = match self {
        Self::Graph(_) => ErrorKind::Graph(error_message),
      };
      error_kind.serialize(serializer)
    }
}

//impl fmt::Debug for Graph {
//    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//        f.debug_struct("Graph")
//            .finish()
//    }
//}


#[derive(Clone)]
pub struct Graph {
    store: Store,
}

impl Graph {
    pub fn open(db: &PathBuf) -> Result<Self, Error> {
        let store = Store::open(db)?;
        Ok(Graph { store })
    }

}