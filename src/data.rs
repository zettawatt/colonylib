use dirs;
use serde;
use serde_json;
use std::fs::{File, create_dir_all, read_to_string, remove_file, write};
use std::io::Error as IoError;
use std::io::Write;
use std::path::PathBuf;
use thiserror;
use tracing::{error, info};

// Import UpdateList from pod module
use crate::pod::UpdateList;

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
        let mut data_dir: PathBuf =
            dirs::data_dir().expect("the data directory path to your OS was not found");
        data_dir.push("colony");

        let downloads_dir: PathBuf = dirs::download_dir().unwrap_or(data_dir.clone());

        let mut pods_dir = data_dir.clone();
        pods_dir.push("pods");

        Self::from_paths(data_dir, pods_dir, downloads_dir)
    }

    pub fn from_paths(
        data_dir: PathBuf,
        pods_dir: PathBuf,
        downloads_dir: PathBuf,
    ) -> Result<Self, Error> {
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
        if !downloads_dir.exists() {
            create_dir_all(&downloads_dir)?;
            info!("Created downloads directory: {:?}", downloads_dir);
        }
        Ok(DataStore {
            data_dir,
            pods_dir,
            downloads_dir,
        })
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
        update_list_path.push("update_list.json");
        update_list_path
    }

    /// Read the current update list from JSON file, creating an empty one if it doesn't exist
    fn read_update_list(&self) -> Result<UpdateList, Error> {
        let update_list_path = self.get_update_list_path();

        if !update_list_path.exists() {
            return Ok(UpdateList::default());
        }

        let contents = read_to_string(&update_list_path)?;
        if contents.trim().is_empty() {
            return Ok(UpdateList::default());
        }

        match serde_json::from_str(&contents) {
            Ok(update_list) => Ok(update_list),
            Err(_) => {
                // If JSON parsing fails, return empty list (could be old format)
                info!("Failed to parse update list as JSON, starting with empty list");
                Ok(UpdateList::default())
            }
        }
    }

    /// Write the update list to JSON file
    fn write_update_list(&self, update_list: &UpdateList) -> Result<(), Error> {
        let update_list_path = self.get_update_list_path();
        let json_content = serde_json::to_string_pretty(update_list)
            .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
        write(&update_list_path, json_content)?;
        Ok(())
    }

    pub fn append_update_list(&self, pod_address: &str) -> Result<(), Error> {
        let mut update_list = self.read_update_list()?;

        // Check if the pod address already exists
        if update_list.pods.contains_key(pod_address) {
            info!("Pod address {} already exists in update list", pod_address);
            return Ok(());
        }

        // Remove from removal list if it exists there (cross-removal behavior)
        if let Some(pos) = update_list
            .remove
            .pointers
            .iter()
            .position(|x| x == pod_address)
        {
            update_list.remove.pointers.remove(pos);
            info!(
                "Removed pod address {} from pointer removal list",
                pod_address
            );
        }

        // Add the pod with an empty scratchpad list (will be populated later)
        update_list.pods.insert(pod_address.to_string(), Vec::new());

        self.write_update_list(&update_list)?;
        Ok(())
    }

    pub fn append_removal_list(&self, address: &str, address_type: &str) -> Result<(), Error> {
        let mut update_list = self.read_update_list()?;

        match address_type {
            "pointer" => {
                if !update_list.remove.pointers.contains(&address.to_string()) {
                    // Remove from pods list if it exists there (cross-removal behavior)
                    if update_list.pods.remove(address).is_some() {
                        info!("Removed pointer address {} from pods update list", address);
                    }

                    update_list.remove.pointers.push(address.to_string());
                } else {
                    info!("Pointer address {} already exists in removal list", address);
                    return Ok(());
                }
            }
            "scratchpad" => {
                if !update_list
                    .remove
                    .scratchpads
                    .contains(&address.to_string())
                {
                    // Remove from any pod's scratchpad list if it exists there (cross-removal behavior)
                    for (pod_address, scratchpads) in update_list.pods.iter_mut() {
                        if let Some(pos) = scratchpads.iter().position(|x| x == address) {
                            scratchpads.remove(pos);
                            info!(
                                "Removed scratchpad address {} from pod {} update list",
                                address, pod_address
                            );
                            break;
                        }
                    }

                    update_list.remove.scratchpads.push(address.to_string());
                } else {
                    info!(
                        "Scratchpad address {} already exists in removal list",
                        address
                    );
                    return Ok(());
                }
            }
            _ => {
                error!("Unknown address type for removal: {}", address_type);
                return Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Unknown address type: {address_type}"),
                )));
            }
        }

        self.write_update_list(&update_list)?;
        Ok(())
    }

    /// Add a scratchpad address to a pod's scratchpad list in the update list
    pub fn add_scratchpad_to_pod(
        &self,
        pod_address: &str,
        scratchpad_address: &str,
    ) -> Result<(), Error> {
        let mut update_list = self.read_update_list()?;

        // Remove scratchpad from removal list if it exists there (cross-removal behavior)
        if let Some(pos) = update_list
            .remove
            .scratchpads
            .iter()
            .position(|x| x == scratchpad_address)
        {
            update_list.remove.scratchpads.remove(pos);
            info!(
                "Removed scratchpad address {} from removal list",
                scratchpad_address
            );
        }

        // Ensure the pod exists in the update list
        let scratchpads = update_list
            .pods
            .entry(pod_address.to_string())
            .or_insert_with(Vec::new);

        // Add the scratchpad if it's not already there
        if !scratchpads.contains(&scratchpad_address.to_string()) {
            scratchpads.push(scratchpad_address.to_string());
        }

        self.write_update_list(&update_list)?;
        Ok(())
    }

    /// Clear the entire update list
    pub fn clear_update_list(&self) -> Result<(), Error> {
        let empty_list = UpdateList::default();
        self.write_update_list(&empty_list)?;
        Ok(())
    }

    /// Get a copy of the current update list
    pub fn get_update_list(&self) -> Result<UpdateList, Error> {
        self.read_update_list()
    }

    pub fn update_pointer_target(
        &self,
        pointer_address: &str,
        scratchpad_address: &str,
    ) -> Result<(), Error> {
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
        writeln!(pointer_file, "{scratchpad_address}")?; // Write the new first line
        writeln!(pointer_file, "{second_line}")?; // Write the second line back

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
        writeln!(pointer_file, "{first_line}")?; // Write the first line back
        writeln!(pointer_file, "{count}")?; // Write the new second line

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

    pub fn remove_pointer_file(&self, address: &str) -> Result<(), Error> {
        let mut pointer_path = self.get_pointers_dir();
        pointer_path.push(address);
        if pointer_path.exists() {
            remove_file(&pointer_path)?;
            info!("Removed pointer file: {:?}", pointer_path);
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

    pub fn remove_scratchpad_file(&self, address: &str) -> Result<(), Error> {
        let mut scratchpad_path = self.get_scratchpads_dir();
        scratchpad_path.push(address);
        if scratchpad_path.exists() {
            remove_file(&scratchpad_path)?;
            info!("Removed scratchpad file: {:?}", scratchpad_path);
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
