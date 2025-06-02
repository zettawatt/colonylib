use std::fs::{File, create_dir_all, read_to_string, write, OpenOptions};
use std::path::PathBuf;
use dirs;
use std::io::Error as IoError;
use tracing::{error, info};
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
        let mut scratchpads_dir = pods_dir.clone();
        scratchpads_dir.push("scratchpads");
        if !scratchpads_dir.exists() {
            create_dir_all(&scratchpads_dir)?;
            info!("Created scratchpads directory: {:?}", scratchpads_dir);
        }
        let mut pointers_dir = pods_dir.clone();
        pointers_dir.push("pointers");
        if !pointers_dir.exists() {
            create_dir_all(&pointers_dir)?;
            info!("Created pointers directory: {:?}", pointers_dir);
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

    pub fn get_pods_dir(&self) -> PathBuf {
        self.pods_dir.clone()
    }

    pub fn get_pointers_dir(&self) -> PathBuf {
        let mut pointers_dir = self.pods_dir.clone();
        pointers_dir.push("pointers");
        pointers_dir
    }

    pub fn get_scratchpads_dir(&self) -> PathBuf {
        let mut scratchpads_dir = self.pods_dir.clone();
        scratchpads_dir.push("scratchpads");
        scratchpads_dir
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

    pub fn get_keystore_path(&self) -> PathBuf {
        let mut keystore_path = self.get_data_path();
        keystore_path.push("keystore.db");
        keystore_path
    }

    pub fn get_graph_path(&self) -> PathBuf {
        let mut graph_path = self.get_data_path();
        graph_path.push("graph.db");
        graph_path
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

    pub fn update_pointer_target(&self, pointer_address: &str, scratchpad_address: &str) -> Result<(), Error> {
        // Update the first line with the scratchpad address, keeping the second line unchanged
        let mut pointer_path = self.get_pointers_dir();
        pointer_path.push(pointer_address);
    
        let mut contents = String::new();
        if pointer_path.exists() {
            contents = read_to_string(&pointer_path)?;
        }
    
        let mut lines = contents.lines();
        let _ = lines.next(); // Skip the first line
        let second_line = lines.next().unwrap_or("0").to_string(); // Get the second line or default to an empty string
    
        let mut pointer_file = File::create(&pointer_path)?; // Overwrite the file
        writeln!(pointer_file, "{}", scratchpad_address)?; // Write the new first line
        writeln!(pointer_file, "{}", second_line)?; // Write the second line back
    
        Ok(())
    }
    
    pub fn update_pointer_count(&self, pointer_address: &str, count: u64) -> Result<(), Error> {
        // Update the second line with the count, keeping the first line unchanged
        let mut pointer_path = self.get_pointers_dir();
        pointer_path.push(pointer_address);
    
        let mut contents = String::new();
        if pointer_path.exists() {
            contents = read_to_string(&pointer_path)?;
        }
    
        let mut lines = contents.lines();
        let first_line = lines.next().unwrap_or("").to_string(); // Get the first line or default to an empty string
    
        let mut pointer_file = File::create(&pointer_path)?; // Overwrite the file
        writeln!(pointer_file, "{}", first_line)?; // Write the first line back
        writeln!(pointer_file, "{}", count)?; // Write the new second line
    
        Ok(())
    }

    pub fn get_pointer_target(&self, pointer_address: &str) -> Result<String, Error> {
        let mut pointer_path = self.get_pointers_dir();
        pointer_path.push(pointer_address);
        let data = read_to_string(pointer_path)?;
        let target = data.lines().next().unwrap_or("").to_string(); // Get the first line or an empty string
        Ok(target)
    }
    pub fn get_pointer_count(&self, pointer_address: &str) -> Result<u64, Error> {
        let mut pointer_path = self.get_pointers_dir();
        pointer_path.push(pointer_address);
        let data = read_to_string(pointer_path)?;
        // get the second line of the file
        let count_line = data.lines().nth(1).unwrap_or("0");
        let count: u64 = count_line.parse().unwrap_or(0);
        Ok(count)
    }

    pub fn get_scratchpad_data(&self, address: &str) -> Result<String, Error> {
        let mut scratchpad_path = self.get_scratchpads_dir();
        scratchpad_path.push(address);
        let data = read_to_string(scratchpad_path)?;
        Ok(data)
    }

    pub fn update_scratchpad_data(&self, address: &str, data: &str) -> Result<(), Error> {
        let mut scratchpad_path = self.get_scratchpads_dir();
        scratchpad_path.push(address);
        write(scratchpad_path, data.as_bytes())?;
        Ok(())
    }

    pub fn create_pointer_file(&self, address: &str) -> Result<(), Error> {
        let mut pointer_path = self.get_pointers_dir();
        pointer_path.push(address);
        if !pointer_path.exists() {
            File::create(&pointer_path)?;
            info!("Created pointer file: {:?}", pointer_path);
        }
        Ok(())
    }

    pub fn create_scratchpad_file(&self, address: &str) -> Result<(), Error> {
        let mut scratchpad_path = self.get_scratchpads_dir();
        scratchpad_path.push(address);
        if !scratchpad_path.exists() {
            File::create(&scratchpad_path)?;
            info!("Created scratchpad file: {:?}", scratchpad_path);
        }
        Ok(())
    }

    pub fn address_is_pointer(&self, address: &str) -> Result<bool, Error> {
        let mut pod_path = self.get_pointers_dir();
        pod_path.push(address);
    
        if pod_path.exists() && pod_path.is_file() {
            return Ok(true);
        }
    
        Ok(false) // Address is not a directory (pointer)
    }

    pub fn address_is_scratchpad(&self, address: &str) -> Result<bool, Error> {
        let mut pod_path = self.get_scratchpads_dir();
        pod_path.push(address); // Append the address to the base directory path
    
        if pod_path.exists() && pod_path.is_file() {
            return Ok(true);
        }
    
        Ok(false)
    }

}



