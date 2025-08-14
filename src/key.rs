use autonomi::client::key_derivation::{DerivationIndex, MainSecretKey};
use autonomi::{PublicKey, SecretKey};
use bip39::Error as Bip39Error;
use bip39::{Language, Mnemonic};
use blsttc::Error as BlsttcError;
use borsh::{BorshDeserialize, BorshSerialize};
use cocoon::Cocoon;
use cocoon::Error as CocoonError;
use hex;
use k256::ecdsa::{SigningKey, VerifyingKey};
use serde;
use sn_bls_ckd::derive_master_sk;
use sn_curv::elliptic::curves::ECScalar;
use std::collections::HashMap;
use std::fmt;
use std::io::Error as IoError;
use thiserror;
use tracing::{debug, error, info, instrument, warn};

// Error handling
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0:?}")]
    Cocoon(CocoonError),
    #[error(transparent)]
    Io(#[from] IoError),
    #[error(transparent)]
    Bip39(#[from] Bip39Error),
    #[error(transparent)]
    Blsttc(#[from] BlsttcError),
    #[error(transparent)]
    Hex(#[from] hex::FromHexError),
    #[error(transparent)]
    K256(#[from] k256::elliptic_curve::Error),
    #[error(transparent)]
    K256Ecdsa(#[from] k256::ecdsa::Error),
}

// Removed manual Display implementation to avoid conflict with thiserror::Error

#[derive(serde::Serialize)]
#[serde(tag = "kind", content = "message")]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    Cocoon(String),
    Io(String),
    Bip39(String),
    Blsttc(String),
    Hex(String),
    K256(String),
    K256Ecdsa(String),
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let error_message = self.to_string();
        let error_kind = match self {
            Self::Cocoon(_) => ErrorKind::Cocoon(error_message),
            Self::Io(_) => ErrorKind::Io(error_message),
            Self::Bip39(_) => ErrorKind::Bip39(error_message),
            Self::Blsttc(_) => ErrorKind::Blsttc(error_message),
            Self::Hex(_) => ErrorKind::Hex(error_message),
            Self::K256(_) => ErrorKind::K256(error_message),
            Self::K256Ecdsa(_) => ErrorKind::K256Ecdsa(error_message),
        };
        error_kind.serialize(serializer)
    }
}

#[derive(BorshDeserialize, BorshSerialize, Clone)]
pub struct KeyStore {
    wallet_key: HashMap<String, Vec<u8>>,
    mnemonic: String,
    main_sk: Vec<u8>,
    pointers: HashMap<Vec<u8>, Vec<u8>>,
    scratchpads: HashMap<Vec<u8>, Vec<u8>>,
    bad_keys: HashMap<Vec<u8>, Vec<u8>>,
    free_pointers: HashMap<Vec<u8>, Vec<u8>>,
    free_scratchpads: HashMap<Vec<u8>, Vec<u8>>,
}

impl fmt::Debug for KeyStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let wallet_keys_debug: HashMap<String, String> = self
            .wallet_key
            .iter()
            .map(|(k, v)| (k.clone(), hex::encode(v)))
            .collect();
        f.debug_struct("KeyStore")
            .field("wallet_key", &wallet_keys_debug)
            .field("mnemonic", &self.mnemonic)
            .field("main_sk", &hex::encode(&self.main_sk))
            .field("pointers", &self.get_pointers())
            .field("scratchpads", &self.get_scratchpads())
            .field("bad_keys", &self.get_bad_keys())
            .finish()
    }
}

impl KeyStore {
    #[instrument]
    pub fn from_file<R: std::io::Read + std::fmt::Debug>(
        file: &mut R,
        password: &str,
    ) -> Result<Self, Error> {
        let cocoon = Cocoon::new(password.as_bytes());
        let encoded = cocoon.parse(file).map_err(Error::Cocoon)?;
        debug!("Read from file: {:?}", file);
        let key_store = KeyStore::try_from_slice(&encoded)?;
        debug!("Parsed key store: {:?}", key_store);
        info!("Key store loaded successfully");
        Ok(key_store)
    }

    #[instrument]
    pub fn to_file<W: std::io::Write + std::fmt::Debug>(
        &self,
        file: &mut W,
        password: &str,
    ) -> Result<(), Error> {
        let mut cocoon = Cocoon::new(password.as_bytes());
        let encoded = borsh::to_vec(&self)?;
        cocoon.dump(encoded, file).map_err(Error::Cocoon)?;
        debug!("Wrote to file: {:?}", file);
        info!("Key store saved successfully");
        Ok(())
    }

    #[instrument]
    pub fn from_mnemonic(mnemonic: &str) -> Result<Self, Error> {
        // Generate a new mnemonic from the given phrase
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, mnemonic)?;
        let seed = mnemonic.to_seed_normalized("");

        // Derive BLS12-381 master secret key from seed using EIP-2333 standard.
        // Guarantees a valid, non-zero scalar represented as 32 Big-Endian bytes.
        let key_bytes: [u8; 32] = derive_master_sk(&seed)
            .expect("derive_master_sk failed; seed length requirement is >= 32 bytes")
            .serialize() // Get the 32-byte Big-Endian representation
            .into(); // Convert GenericArray<u8, 32> to [u8; 32]

        // Create a SecretKey from the 32-byte array
        let secret_key = SecretKey::from_bytes(key_bytes)?;

        let hex_key = hex::encode(secret_key.to_bytes());
        let hex_key = hex_key.as_str();

        Self::from_hex(hex_key, mnemonic.to_string().as_str())
    }

    #[instrument]
    pub fn from_hex(key: &str, mnemonic: &str) -> Result<Self, Error> {
        let secret_key = SecretKey::from_hex(key)?;

        // Generate a new main keys from the mnemonic
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        //let main_pk: MainPubkey = main_sk.public_key();

        // Create a new pods hashmap
        let mut pointers: HashMap<PublicKey, SecretKey> = HashMap::new();
        let mut scratchpads: HashMap<PublicKey, SecretKey> = HashMap::new();
        let bad_keys: HashMap<PublicKey, SecretKey> = HashMap::new();
        let free_pointers: HashMap<PublicKey, SecretKey> = HashMap::new();
        let free_scratchpads: HashMap<PublicKey, SecretKey> = HashMap::new();
        //let pod_key: SecretKey = main_sk.derive_key(&index(0)).into();
        //let pod_pubkey: PublicKey = pod_key.public_key();
        //pods.insert(pod_pubkey, pod_key.clone());

        // Add configuratino pod pointer key
        let pointer_key: SecretKey = main_sk.derive_key(&index(0)).into();
        let pointer_pubkey: PublicKey = pointer_key.clone().public_key();
        pointers.insert(pointer_pubkey, pointer_key);

        // Add configuration pod scratchpad key
        let scratchpad_key: SecretKey = main_sk.derive_key(&index(1)).into();
        let scratchpad_pubkey: PublicKey = scratchpad_key.clone().public_key();
        scratchpads.insert(scratchpad_pubkey, scratchpad_key);

        Ok(KeyStore {
            wallet_key: HashMap::new(),
            mnemonic: mnemonic.to_string(),
            main_sk: main_sk.to_bytes(),
            pointers: pointers
                .iter()
                .map(|(k, v)| (k.to_bytes().to_vec(), v.to_bytes().to_vec()))
                .collect(),
            scratchpads: scratchpads
                .iter()
                .map(|(k, v)| (k.to_bytes().to_vec(), v.to_bytes().to_vec()))
                .collect(),
            bad_keys: bad_keys
                .iter()
                .map(|(k, v)| (k.to_bytes().to_vec(), v.to_bytes().to_vec()))
                .collect(),
            free_pointers: free_pointers
                .iter()
                .map(|(k, v)| (k.to_bytes().to_vec(), v.to_bytes().to_vec()))
                .collect(),
            free_scratchpads: free_scratchpads
                .iter()
                .map(|(k, v)| (k.to_bytes().to_vec(), v.to_bytes().to_vec()))
                .collect(),
        })
    }

    pub fn get_seed_phrase(&self) -> String {
        debug!("Seed phrase: {}", self.mnemonic);
        self.mnemonic.clone()
    }

    pub fn add_wallet_key(&mut self, name: &str, wallet_key: &str) -> Result<(), Error> {
        let wallet_key = remove_0x_prefix(wallet_key);
        // Verify that the decoded key is a valid Ethereum private key (32 bytes)
        let decoded_key = hex::decode(&wallet_key)?;
        if decoded_key.len() != 32 {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Ethereum private key must be exactly 32 bytes",
            )));
        }
        // Verify it's a valid secp256k1 private key
        let _signing_key = SigningKey::from_slice(&decoded_key)?;

        self.wallet_key
            .insert(name.to_string(), decoded_key.clone());
        debug!(
            "Wallet key added for '{}': {}",
            name,
            hex::encode(decoded_key)
        );
        Ok(())
    }

    pub fn remove_wallet_key(&mut self, name: &str) -> Result<(), Error> {
        match self.wallet_key.remove(name) {
            Some(_) => {
                debug!("Wallet key removed for '{}'", name);
                Ok(())
            }
            None => Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Wallet key '{name}' not found"),
            ))),
        }
    }

    pub fn get_wallet_key(&self, name: &str) -> Result<String, Error> {
        match self.wallet_key.get(name) {
            Some(key) => {
                let encoded_key = hex::encode(key);
                debug!("Wallet key for '{}': {}", name, encoded_key);
                Ok(encoded_key)
            }
            None => Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Wallet key '{name}' not found"),
            ))),
        }
    }

    pub fn get_wallet_keys(&self) -> HashMap<String, String> {
        self.wallet_key
            .iter()
            .map(|(k, v)| (k.clone(), hex::encode(v)))
            .collect()
    }

    pub fn set_active_wallet(&mut self, name: &str) -> Result<(String, String), Error> {
        if !self.wallet_key.contains_key(name) {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Wallet key '{name}' not found"),
            )));
        }
        let active_wallet = name;
        let active_wallet_address = self.get_wallet_address(name)?;
        debug!("Active wallet set to '{}'", name);
        Ok((active_wallet.to_string(), active_wallet_address))
    }

    pub fn get_wallet_address(&self, name: &str) -> Result<String, Error> {
        match self.wallet_key.get(name) {
            Some(key_bytes) => {
                if key_bytes.len() == 32 {
                    match SigningKey::from_slice(key_bytes) {
                        Ok(signing_key) => {
                            let verifying_key = signing_key.verifying_key();
                            let address = ethereum_address_from_public_key(verifying_key);
                            debug!("Wallet address for '{}': {}", name, address);
                            Ok(address)
                        }
                        Err(e) => {
                            warn!(
                                "Invalid wallet key for '{}': {}. Using default address.",
                                name, e
                            );
                            // Return a default Ethereum address (all zeros)
                            Ok("0x0000000000000000000000000000000000000000".to_string())
                        }
                    }
                } else {
                    warn!(
                        "Invalid wallet key length for '{}' (expected 32 bytes, got {}). Using default address.",
                        name,
                        key_bytes.len()
                    );
                    // Return a default Ethereum address (all zeros)
                    Ok("0x0000000000000000000000000000000000000000".to_string())
                }
            }
            None => Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Wallet key '{name}' not found"),
            ))),
        }
    }

    pub fn get_wallet_addresses(&self) -> HashMap<String, String> {
        // Get the list of wallet keys
        let wallet_keys = self.get_wallet_keys();

        // Return the list of wallet addresses by deriving them from the wallet keys
        wallet_keys
            .keys()
            .map(|k| {
                // Derive the Ethereum address from the wallet key
                let address = self
                    .get_wallet_address(k.as_str())
                    .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string());
                (k.clone(), address)
            })
            .collect()
    }

    pub fn get_configuration_address(&self) -> Result<String, Error> {
        // Get the first derived key
        let main_sk_array: [u8; 32] = self
            .main_sk
            .clone()
            .try_into()
            .expect("main_sk must be 32 bytes");
        let secret_key: SecretKey = SecretKey::from_bytes(main_sk_array)?;
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        let key: SecretKey = main_sk.derive_key(&index(0)).into();
        let pubkey: PublicKey = key.clone().public_key();
        debug!("Configuration pod address: {}", pubkey.to_hex());
        Ok(pubkey.to_hex())
    }

    pub fn get_configuration_scratchpad_address(&self) -> Result<String, Error> {
        // Get the first derived key
        let main_sk_array: [u8; 32] = self
            .main_sk
            .clone()
            .try_into()
            .expect("main_sk must be 32 bytes");
        let secret_key: SecretKey = SecretKey::from_bytes(main_sk_array)?;
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        let key: SecretKey = main_sk.derive_key(&index(1)).into();
        let pubkey: PublicKey = key.clone().public_key();
        debug!("Configuration pod address: {}", pubkey.to_hex());
        Ok(pubkey.to_hex())
    }

    pub fn get_pointer_key(&self, pod_pubkey: String) -> Result<String, Error> {
        let decoded_key = hex::decode(pod_pubkey.as_str())?;
        match self.pointers.get(&decoded_key) {
            Some(value) => {
                debug!("Pointer key: {}", hex::encode(value));
                Ok(hex::encode(value))
            }
            None => Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Key not found",
            ))),
        }
    }

    pub fn get_scratchpad_key(&self, pod_pubkey: String) -> Result<String, Error> {
        let decoded_key = hex::decode(pod_pubkey.as_str())?;
        match self.scratchpads.get(&decoded_key) {
            Some(value) => {
                debug!("Scratchpad key: {}", hex::encode(value));
                Ok(hex::encode(value))
            }
            None => Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Key not found",
            ))),
        }
    }

    pub fn get_free_pointer_key(&self, pod_pubkey: String) -> Result<String, Error> {
        let decoded_key = hex::decode(pod_pubkey.as_str())?;
        match self.free_pointers.get(&decoded_key) {
            Some(value) => {
                debug!("Pointer key: {}", hex::encode(value));
                Ok(hex::encode(value))
            }
            None => Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Key not found",
            ))),
        }
    }

    pub fn get_free_scratchpad_key(&self, pod_pubkey: String) -> Result<String, Error> {
        let decoded_key = hex::decode(pod_pubkey.as_str())?;
        match self.free_scratchpads.get(&decoded_key) {
            Some(value) => {
                debug!("Scratchpad key: {}", hex::encode(value));
                Ok(hex::encode(value))
            }
            None => Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Key not found",
            ))),
        }
    }

    pub fn get_bad_key(&self, pod_pubkey: String) -> Result<String, Error> {
        let decoded_key = hex::decode(pod_pubkey.as_str())?;
        match self.bad_keys.get(&decoded_key) {
            Some(value) => {
                debug!("Bad key: {}", hex::encode(value));
                Ok(hex::encode(value))
            }
            None => Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Key not found",
            ))),
        }
    }

    pub fn address_is_pointer(&self, address: &str) -> bool {
        self.pointers.contains_key(&hex::decode(address).unwrap_or_default())
    }    

    pub fn add_pointer_key(&mut self) -> Result<(String, String), Error> {
        // Check for unused keys first
        let key_pair = self.free_pointers.iter().next();
        if let Some((pubkey, key)) = key_pair {
            let pubkey = pubkey.clone();
            let key = key.clone();
            self.free_pointers.remove(&pubkey);
            self.pointers.insert(pubkey.clone(), key.clone());
            debug!(
                "Reusing unused key at address: {}",
                hex::encode(pubkey.clone())
            );
            return Ok((hex::encode(pubkey), hex::encode(key)));
        }

        let main_sk_array: [u8; 32] = self
            .main_sk
            .clone()
            .try_into()
            .expect("main_sk must be 32 bytes");
        let secret_key: SecretKey = SecretKey::from_bytes(main_sk_array)?;
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        let num_keys = self.get_num_keys();
        let pod_key: SecretKey = main_sk.derive_key(&index(num_keys)).into();
        let pod_pubkey: PublicKey = pod_key.clone().public_key();
        self.pointers.insert(
            pod_pubkey.to_bytes().to_vec(),
            pod_key.clone().to_bytes().to_vec(),
        );
        Ok((
            pod_pubkey.to_hex().to_string(),
            pod_key.to_hex().to_string(),
        ))
    }

    pub fn remove_pointer_key(&mut self, address: &str) -> Result<(), Error> {
        let pubkey = hex::decode(address)?;
        let key = self.pointers.remove(&pubkey);
        match key {
            Some(value) => {
                self.free_pointers.insert(pubkey, value);
            }
            None => {
                return Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Key not found",
                )));
            }
        }
        Ok(())
    }

    pub fn add_scratchpad_key(&mut self) -> Result<(String, String), Error> {
        // Check for unused keys first
        let key_pair = self.free_scratchpads.iter().next();
        if let Some((pubkey, key)) = key_pair {
            let pubkey = pubkey.clone();
            let key = key.clone();
            self.free_scratchpads.remove(&pubkey);
            self.scratchpads.insert(pubkey.clone(), key.clone());
            debug!(
                "Reusing unused key at address: {}",
                hex::encode(pubkey.clone())
            );
            return Ok((hex::encode(pubkey), hex::encode(key)));
        }
        let main_sk_array: [u8; 32] = self
            .main_sk
            .clone()
            .try_into()
            .expect("main_sk must be 32 bytes");
        let secret_key: SecretKey = SecretKey::from_bytes(main_sk_array)?;
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        let num_keys = self.get_num_keys();
        let pod_key: SecretKey = main_sk.derive_key(&index(num_keys)).into();
        let pod_pubkey: PublicKey = pod_key.clone().public_key();
        self.scratchpads.insert(
            pod_pubkey.to_bytes().to_vec(),
            pod_key.clone().to_bytes().to_vec(),
        );
        Ok((
            pod_pubkey.to_hex().to_string(),
            pod_key.to_hex().to_string(),
        ))
    }

    pub fn remove_scratchpad_key(&mut self, address: &str) -> Result<(), Error> {
        let pubkey = hex::decode(address)?;
        let key = self.scratchpads.remove(&pubkey);
        match key {
            Some(value) => {
                self.free_scratchpads.insert(pubkey, value);
            }
            None => {
                return Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Key not found",
                )));
            }
        }
        Ok(())
    }

    pub fn add_bad_key(&mut self) -> Result<String, Error> {
        let main_sk_array: [u8; 32] = self
            .main_sk
            .clone()
            .try_into()
            .expect("main_sk must be 32 bytes");
        let secret_key: SecretKey = SecretKey::from_bytes(main_sk_array)?;
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        let num_keys = self.get_num_keys();
        let pod_key: SecretKey = main_sk.derive_key(&index(num_keys)).into();
        let pod_pubkey: PublicKey = pod_key.clone().public_key();
        self.bad_keys.insert(
            pod_pubkey.to_bytes().to_vec(),
            pod_key.clone().to_bytes().to_vec(),
        );
        Ok(pod_key.to_hex().to_string())
    }

    pub fn add_free_pointer_key(&mut self) -> Result<String, Error> {
        let main_sk_array: [u8; 32] = self
            .main_sk
            .clone()
            .try_into()
            .expect("main_sk must be 32 bytes");
        let secret_key: SecretKey = SecretKey::from_bytes(main_sk_array)?;
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        let num_keys = self.get_num_keys();
        let pod_key: SecretKey = main_sk.derive_key(&index(num_keys)).into();
        let pod_pubkey: PublicKey = pod_key.clone().public_key();
        self.free_pointers.insert(
            pod_pubkey.to_bytes().to_vec(),
            pod_key.clone().to_bytes().to_vec(),
        );
        Ok(pod_key.to_hex().to_string())
    }

    pub fn add_free_scratchpad_key(&mut self) -> Result<String, Error> {
        let main_sk_array: [u8; 32] = self
            .main_sk
            .clone()
            .try_into()
            .expect("main_sk must be 32 bytes");
        let secret_key: SecretKey = SecretKey::from_bytes(main_sk_array)?;
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        let num_keys = self.get_num_keys();
        let pod_key: SecretKey = main_sk.derive_key(&index(num_keys)).into();
        let pod_pubkey: PublicKey = pod_key.clone().public_key();
        self.free_scratchpads.insert(
            pod_pubkey.to_bytes().to_vec(),
            pod_key.clone().to_bytes().to_vec(),
        );
        Ok(pod_key.to_hex().to_string())
    }

    pub fn clear_keys(&mut self) -> Result<(), Error> {
        self.pointers.clear();
        self.scratchpads.clear();
        self.bad_keys.clear();
        self.free_pointers.clear();
        self.free_scratchpads.clear();
        Ok(())
    }

    pub fn get_num_pointer_keys(&self) -> u64 {
        debug!("Number of pointer keys: {}", self.pointers.len());
        self.pointers.len() as u64
    }

    pub fn get_num_scratchpad_keys(&self) -> u64 {
        debug!("Number of scratchpad keys: {}", self.scratchpads.len());
        self.scratchpads.len() as u64
    }

    pub fn get_num_bad_keys(&self) -> u64 {
        debug!("Number of bad derived keys: {}", self.bad_keys.len());
        self.bad_keys.len() as u64
    }

    pub fn get_num_keys(&self) -> u64 {
        self.get_num_pointer_keys() + self.get_num_scratchpad_keys() + self.get_num_bad_keys()
    }

    pub fn get_pointers(&self) -> HashMap<String, String> {
        self.pointers
            .iter()
            .map(|(k, v)| (hex::encode(k), hex::encode(v)))
            .collect()
    }

    pub fn get_scratchpads(&self) -> HashMap<String, String> {
        self.scratchpads
            .iter()
            .map(|(k, v)| (hex::encode(k), hex::encode(v)))
            .collect()
    }

    pub fn get_bad_keys(&self) -> HashMap<String, String> {
        self.bad_keys
            .iter()
            .map(|(k, v)| (hex::encode(k), hex::encode(v)))
            .collect()
    }

    pub fn get_free_pointers(&self) -> HashMap<String, String> {
        self.free_pointers
            .iter()
            .map(|(k, v)| (hex::encode(k), hex::encode(v)))
            .collect()
    }

    pub fn get_free_scratchpads(&self) -> HashMap<String, String> {
        self.free_scratchpads
            .iter()
            .map(|(k, v)| (hex::encode(k), hex::encode(v)))
            .collect()
    }

    pub fn get_address_at_index(&self, count: u64) -> Result<String, Error> {
        let main_sk_array: [u8; 32] = self
            .main_sk
            .clone()
            .try_into()
            .expect("main_sk must be 32 bytes");
        let secret_key: SecretKey = SecretKey::from_bytes(main_sk_array)?;
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        let pod_key: SecretKey = main_sk.derive_key(&index(count)).into();
        Ok(pod_key.public_key().to_hex())
    }
}

fn index(i: u64) -> DerivationIndex {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&i.to_ne_bytes());
    DerivationIndex::from_bytes(bytes)
}

fn remove_0x_prefix(input: &str) -> String {
    input.strip_prefix("0x").unwrap_or(input).to_string()
}

fn ethereum_address_from_public_key(verifying_key: &VerifyingKey) -> String {
    use sha3::{Digest, Keccak256};

    // Get the uncompressed public key (65 bytes: 0x04 + 32 bytes x + 32 bytes y)
    let public_key_bytes = verifying_key.to_encoded_point(false);
    let public_key_bytes = public_key_bytes.as_bytes();

    // Skip the first byte (0x04) and hash the remaining 64 bytes
    let hash = Keccak256::digest(&public_key_bytes[1..]);

    // Take the last 20 bytes and format as hex with 0x prefix
    let address_bytes = &hash[12..];
    format!("0x{}", hex::encode(address_bytes))
}
