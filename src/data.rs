use std::fs::{File, create_dir_all, read_to_string, write, OpenOptions};
use std::path::PathBuf;
use dirs;
use std::io::Error as IoError;
use tracing::{debug, error, info, warn, instrument};
use std::io::Write;
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
        pod_refs_dir.push("pod_refs");
  
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

    pub fn get_pod_path(&self, address: &str) -> PathBuf {
        let mut pod_path = self.pods_dir.clone();
        pod_path.push(address);
        pod_path
    }

    pub fn get_pod_ref_path(&self, address: &str) -> PathBuf {
        let mut pod_ref_path = self.pod_refs_dir.clone();
        pod_ref_path.push(address);
        pod_ref_path
    }

    pub fn get_downloads_path(&self) -> PathBuf {
        self.downloads_dir.clone()
    }

    pub fn get_data_path(&self) -> PathBuf {
        self.data_dir.clone()
    }

    pub fn write_pod(&self, address: &str, data: &str) -> Result<(), Error> {
        let pod_path = self.get_pod_path(address);
        write(pod_path, data.as_bytes())?;
        Ok(())
    }

    pub fn read_pod(&self, address: &str) -> Result<String, Error> {
        let pod_path = self.get_pod_path(address);
        let data = read_to_string(pod_path)?;
        Ok(data)
    }

    pub fn get_update_list_path(&self) -> PathBuf {
        let mut update_list_path = self.get_data_path();
        update_list_path.push("update_list.txt");
        update_list_path
    }

    pub fn append_update_list(&self, address: &str) -> Result<(), Error> {
        let update_list_path = self.get_update_list_path();
        let mut update_list = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&update_list_path)?;

        // Check if the file has a line that matches the address
        let contents = read_to_string(&update_list_path)?;
        if contents.lines().any(|line| line == address) {
            info!("Address {} already exists in update list", address);
            return Ok(());
        }
        // If not, append the address to the file
        writeln!(update_list, "{}", address)?;
        Ok(())
    }

    pub fn address_is_scratchpad(&self, address: &str) -> Result<bool, Error> {
        let pod_path = self.pods_dir.clone(); // Get the base pod directory
        if pod_path.exists() && pod_path.is_dir() {
            for entry in std::fs::read_dir(pod_path)? {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        let mut file_path = path.clone();
                        file_path.push(address);
                        if file_path.exists() && file_path.is_file() {
                            return Ok(true); // Address is a scratchpad file
                        }
                    }
                }
            }
        }
        Ok(false) // Address is not a scratchpad file
    }

    pub fn address_is_pointer(&self, address: &str) -> Result<bool, Error> {
        let mut pod_path = self.pods_dir.clone(); // Get the base pod directory
        pod_path.push(address); // Append the address to the base directory path
    
        if pod_path.exists() && pod_path.is_dir() {
            return Ok(true); // Address is a directory (pointer)
        }
    
        Ok(false) // Address is not a directory (pointer)
    }
}

