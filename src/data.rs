use std::fs::{File, create_dir_all, read_to_string, write};
use std::path::PathBuf;
use dirs;
use std::io::Error as IoError;
use tracing::{debug, error, info, warn, instrument};
use thiserror;
use serde;

// Error handling
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] IoError),
}

#[derive(serde::Serialize)]
#[serde(tag = "kind", content = "message")]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    Io(String),
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
      S: serde::ser::Serializer,
    {
      let error_message = self.to_string();
      let error_kind = match self {
        Self::Io(_) => ErrorKind::Io(error_message),
      };
      error_kind.serialize(serializer)
    }
  }

#[derive(Clone, Debug)]
pub struct DataStore {
    data_dir: PathBuf,
    pods_dir: PathBuf,
    downloads_dir: PathBuf,
}

impl DataStore {
    pub fn create() -> Result<Self, Error> {

        // Create the data directory if it doesn't exist
        let mut data_dir: PathBuf = dirs::data_dir().expect("the data directory path to your OS was not found");
        data_dir.push("colony");
        if !data_dir.exists() {
            create_dir_all(&data_dir)?;
        }
  
        let downloads_dir: PathBuf = dirs::download_dir().unwrap_or(data_dir.clone());
        
        // Create the pods directory if it doesn't exist
        let mut pods_dir = data_dir.clone();
        pods_dir.push("pods");
        if !pods_dir.exists() {
            create_dir_all(&pods_dir)?;
        }
  
        Ok(DataStore { data_dir, pods_dir, downloads_dir })
    }

    pub fn get_pod_path(&self, pod_id: &str) -> PathBuf {
        let mut pod_path = self.pods_dir.clone();
        pod_path.push(pod_id);
        pod_path
    }

    pub fn write_pod_data(&self, pod_id: &str, data: &str) -> Result<(), Error> {
        let pod_path = self.get_pod_path(pod_id);
        write(pod_path, data.as_bytes())?;
        Ok(())
    }

    pub fn read_pod_data(&self, pod_id: &str) -> Result<String, Error> {
        let pod_path = self.get_pod_path(pod_id);
        let data = read_to_string(pod_path)?;
        Ok(data)
    }
}

