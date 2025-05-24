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
    pod_refs_dir: PathBuf,
    downloads_dir: PathBuf,
}

impl DataStore {
    pub fn create() -> Result<Self, Error> {

        let mut data_dir: PathBuf = dirs::data_dir().expect("the data directory path to your OS was not found");
        data_dir.push("colony");

        let downloads_dir: PathBuf = dirs::download_dir().unwrap_or(data_dir.clone());
        
        let mut pods_dir = data_dir.clone();
        pods_dir.push("pods");
  
        let mut pod_refs_dir = data_dir.clone();
        pod_refs_dir.push("pods");
  
        Ok(Self::from_paths(data_dir, pods_dir, pod_refs_dir, downloads_dir)?)
    }

    pub fn from_paths(data_dir: PathBuf, pods_dir: PathBuf, pod_refs_dir: PathBuf, downloads_dir: PathBuf) -> Result<Self, Error> {
        if !data_dir.exists() {
            create_dir_all(&data_dir)?;
            info!("Created data directory: {:?}", data_dir);
        }
        if !pods_dir.exists() {
            create_dir_all(&pods_dir)?;
            info!("Created pods directory: {:?}", pods_dir);
        }
        if !pod_refs_dir.exists() {
            create_dir_all(&pod_refs_dir)?;
            info!("Created pod reference directory: {:?}", pod_refs_dir);
        }
        if !downloads_dir.exists() {
            create_dir_all(&downloads_dir)?;
            info!("Created downloads directory: {:?}", downloads_dir);
        }
        Ok(DataStore { data_dir, pods_dir, pod_refs_dir, downloads_dir })
    }

    pub fn get_path(&self, pod_id: &str) -> PathBuf {
        let mut pod_path = self.pods_dir.clone();
        pod_path.push(pod_id);
        pod_path
    }

    pub fn write(&self, pod_id: &str, data: &str) -> Result<(), Error> {
        let pod_path = self.get_path(pod_id);
        write(pod_path, data.as_bytes())?;
        Ok(())
    }

    pub fn read(&self, pod_id: &str) -> Result<String, Error> {
        let pod_path = self.get_path(pod_id);
        let data = read_to_string(pod_path)?;
        Ok(data)
    }
}

