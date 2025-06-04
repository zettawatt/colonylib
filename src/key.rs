use borsh::{BorshDeserialize, BorshSerialize};
use bip39::{Mnemonic, Language};
use bip39::Error as Bip39Error;
use autonomi::client::key_derivation::{DerivationIndex, MainSecretKey};
use autonomi::{SecretKey, PublicKey};
use cocoon::Cocoon;
use cocoon::Error as CocoonError;
use std::collections::HashMap;
use std::io::Error as IoError;
use std::fmt;
use blsttc::Error as BlsttcError;
use sn_bls_ckd::derive_master_sk;
use sn_curv::elliptic::curves::ECScalar;
use hex;
use tracing::{debug, error, info, warn, instrument};
use thiserror;
use serde;

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
      };
      error_kind.serialize(serializer)
    }
  }


#[derive(BorshDeserialize, BorshSerialize, Clone)]
pub struct KeyStore {
    wallet_key: Vec<u8>,
    mnemonic: String,
    main_sk: Vec<u8>,
    pointers: HashMap<Vec<u8>, Vec<u8>>,
    scratchpads: HashMap<Vec<u8>, Vec<u8>>,
    bad_keys: HashMap<Vec<u8>, Vec<u8>>,
}

impl fmt::Debug for KeyStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyStore")
            .field("wallet_key", &hex::encode(&self.wallet_key))
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
    pub fn from_file<R: std::io::Read + std::fmt::Debug>(file: &mut R, password: &str) -> Result<Self, Error> {
        let cocoon = Cocoon::new(&password.as_bytes());
        let encoded = cocoon.parse(file).map_err(Error::Cocoon)?;
        debug!("Read from file: {:?}", file);
        let key_store = KeyStore::try_from_slice(&encoded)?;
        debug!("Parsed key store: {:?}", key_store);
        info!("Key store loaded successfully");
        Ok(key_store)
    }

    #[instrument]
    pub fn to_file<W: std::io::Write + std::fmt::Debug>(&self, file: &mut W, password: &str) -> Result<(), Error> {
        let mut cocoon = Cocoon::new(&password.as_bytes());
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

        Ok(Self::from_hex(hex_key, mnemonic.to_string().as_str())?)
    }

    #[instrument]
    pub fn from_hex(key: &str, mnemonic: &str) -> Result<Self, Error> {

        let secret_key = SecretKey::from_hex(key)?;

        // Generate a new main keys from the mnemonic
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        //let main_pk: MainPubkey = main_sk.public_key();

        // Create a new pods hashmap
        let pointers: HashMap<PublicKey, SecretKey> = HashMap::new();
        let scratchpads: HashMap<PublicKey, SecretKey> = HashMap::new();
        let bad_keys: HashMap<PublicKey, SecretKey> = HashMap::new();
        //let pod_key: SecretKey = main_sk.derive_key(&index(0)).into();
        //let pod_pubkey: PublicKey = pod_key.public_key();
        //pods.insert(pod_pubkey, pod_key.clone());

        Ok(KeyStore {
            wallet_key: SecretKey::default().to_bytes().to_vec(),
            mnemonic: mnemonic.to_string(),
            main_sk: main_sk.to_bytes(),
            pointers: pointers.iter().map(|(k, v)| (k.to_bytes().to_vec(), v.to_bytes().to_vec())).collect(),
            scratchpads: scratchpads.iter().map(|(k, v)| (k.to_bytes().to_vec(), v.to_bytes().to_vec())).collect(),
            bad_keys: bad_keys.iter().map(|(k, v)| (k.to_bytes().to_vec(), v.to_bytes().to_vec())).collect()
        })
    }

    pub fn get_seed_phrase(&self) -> String {
        debug!("Seed phrase: {}", self.mnemonic);
        self.mnemonic.clone()
    }

    pub fn set_wallet_key(&mut self, wallet_key: String) -> Result<(), Error> {
        let wallet_key = remove_0x_prefix(wallet_key.as_str());
        self.wallet_key = hex::decode(wallet_key)?;
        debug!("Wallet key set: {}", hex::encode(self.wallet_key.clone()));
        Ok(())
    }

    pub fn get_wallet_key(&self) -> String {
        debug!("Wallet key: {}", hex::encode(self.wallet_key.clone()));
        hex::encode(self.wallet_key.clone())
    }

    pub fn get_pointer_key(&self, pod_pubkey: String) -> Result<String, Error> {
        let decoded_key = hex::decode(pod_pubkey.as_str())?;
        match self.pointers.get(&decoded_key) {
            Some(value) => {
                debug!("Pointer key: {}", hex::encode(value));
                Ok(hex::encode(value))
            },
            None => Err(Error::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "Key not found"))),
        }
    }

    pub fn get_scratchpad_key(&self, pod_pubkey: String) -> Result<String, Error> {
        let decoded_key = hex::decode(pod_pubkey.as_str())?;
        match self.scratchpads.get(&decoded_key) {
            Some(value) => {
                debug!("Scratchpad key: {}", hex::encode(value));
                Ok(hex::encode(value))
            },
            None => Err(Error::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "Key not found"))),
        }
    }

    pub fn get_bad_key(&self, pod_pubkey: String) -> Result<String, Error> {
        let decoded_key = hex::decode(pod_pubkey.as_str())?;
        match self.bad_keys.get(&decoded_key) {
            Some(value) => {
                debug!("Bad key: {}", hex::encode(value));
                Ok(hex::encode(value))
            },
            None => Err(Error::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "Key not found"))),
        }
    }

    pub fn add_pointer_key(&mut self) -> Result<String, Error> {
        let main_sk_array: [u8; 32] = self.main_sk.clone().try_into().expect("main_sk must be 32 bytes");
        let secret_key: SecretKey = SecretKey::from_bytes(main_sk_array)?;
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        let num_keys = self.get_num_keys();
        let pod_key: SecretKey = main_sk.derive_key(&index(num_keys)).into();
        let pod_pubkey: PublicKey = pod_key.clone().public_key();
        self.pointers.insert(pod_pubkey.to_bytes().to_vec(), pod_key.clone().to_bytes().to_vec());
        Ok(pod_key.to_hex().to_string())
    }

    pub fn add_scratchpad_key(&mut self) -> Result<String, Error> {
        let main_sk_array: [u8; 32] = self.main_sk.clone().try_into().expect("main_sk must be 32 bytes");
        let secret_key: SecretKey = SecretKey::from_bytes(main_sk_array)?;
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        let num_keys = self.get_num_keys();
        let pod_key: SecretKey = main_sk.derive_key(&index(num_keys)).into();
        let pod_pubkey: PublicKey = pod_key.clone().public_key();
        self.scratchpads.insert(pod_pubkey.to_bytes().to_vec(), pod_key.clone().to_bytes().to_vec());
        Ok(pod_key.to_hex().to_string())
    }

    pub fn add_bad_key(&mut self) -> Result<String, Error> {
        let main_sk_array: [u8; 32] = self.main_sk.clone().try_into().expect("main_sk must be 32 bytes");
        let secret_key: SecretKey = SecretKey::from_bytes(main_sk_array)?;
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        let num_keys = self.get_num_keys();
        let pod_key: SecretKey = main_sk.derive_key(&index(num_keys)).into();
        let pod_pubkey: PublicKey = pod_key.clone().public_key();
        self.bad_keys.insert(pod_pubkey.to_bytes().to_vec(), pod_key.clone().to_bytes().to_vec());
        Ok(pod_key.to_hex().to_string())
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
        self.pointers.iter().map(|(k, v)| (hex::encode(k), hex::encode(v))).collect()
    }

    pub fn get_scratchpads(&self) -> HashMap<String, String> {
        self.scratchpads.iter().map(|(k, v)| (hex::encode(k), hex::encode(v))).collect()
    }

    pub fn get_bad_keys(&self) -> HashMap<String, String> {
        self.bad_keys.iter().map(|(k, v)| (hex::encode(k), hex::encode(v))).collect()
    }

    pub fn get_address_at_index(&self, count: u64) -> Result<String, Error> {
        let main_sk_array: [u8; 32] = self.main_sk.clone().try_into().expect("main_sk must be 32 bytes");
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
    if input.starts_with("0x") {
        input[2..].to_string()
    } else {
        input.to_string()
    }
}


