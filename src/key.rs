use borsh::{BorshDeserialize, BorshSerialize};
use bip39::{Mnemonic, Language};
use bip39::Error as Bip39Error;
use autonomi::client::key_derivation::{DerivationIndex, MainSecretKey};
use autonomi::{SecretKey, PublicKey};
use cocoon::Cocoon;
use cocoon::Error as CocoonError;
use std::collections::HashMap;
use std::io::Error as IoError;
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


#[derive(BorshDeserialize, BorshSerialize, Clone, Debug)]
pub struct KeyStore {
    wallet_key: Vec<u8>,
    mnemonic: String,
    main_sk: Vec<u8>,
    pods: HashMap<Vec<u8>, Vec<u8>>,
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
    pub fn from_mnemonic(mnemonic: String) -> Result<Self, Error> {

        // Generate a new mnemonic from the given phrase
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, mnemonic.as_str())?;
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

        Ok(Self::from_hex(hex_key)?)
    }

    #[instrument]
    pub fn from_hex(key: String) -> Result<Self, Error> {

        let secret_key = SecretKey::from_hex(key.as_str())?;

        // Generate a new main keys from the mnemonic
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        //let main_pk: MainPubkey = main_sk.public_key();

        // Create a new pods hashmap
        let pods: HashMap<PublicKey, SecretKey> = HashMap::new();
        //let pod_key: SecretKey = main_sk.derive_key(&index(0)).into();
        //let pod_pubkey: PublicKey = pod_key.public_key();
        //pods.insert(pod_pubkey, pod_key.clone());

        Ok(KeyStore {
            wallet_key: SecretKey::default().to_bytes().to_vec(),
            mnemonic: "Unknown, initialized with key".to_string(),
            main_sk: main_sk.to_bytes(),
            pods: pods.iter().map(|(k, v)| (k.to_bytes().to_vec(), v.to_bytes().to_vec())).collect()
        })
    }

    #[instrument]
    pub fn get_seed_phrase(&self) -> String {
        debug!("Seed phrase: {}", self.mnemonic);
        self.mnemonic.clone()
    }

    #[instrument]
    pub fn set_wallet_key(&mut self, wallet_key: String) -> Result<(), Error> {
        let wallet_key = remove_0x_prefix(wallet_key.as_str());
        self.wallet_key = hex::decode(wallet_key)?;
        debug!("Wallet key set: {}", hex::encode(self.wallet_key.clone()));
        Ok(())
    }

    #[instrument]
    pub fn get_wallet_key(&self) -> String {
        debug!("Wallet key: {}", hex::encode(self.wallet_key.clone()));
        hex::encode(self.wallet_key.clone())
    }

    #[instrument]
    pub fn get_pod_key(&self, pod_pubkey: String) -> Result<String, Error> {
        let decoded_key = hex::decode(pod_pubkey.as_str())?;
        match self.pods.get(&decoded_key) {
            Some(value) => {
                debug!("Pod key: {}", hex::encode(value));
                Ok(hex::encode(value))
            },
            None => Err(Error::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "Key not found"))),
        }
    }

    #[instrument]
    pub fn add_derived_key(&mut self) -> Result<String, Error> {
        let main_sk_array: [u8; 32] = self.main_sk.clone().try_into().expect("main_sk must be 32 bytes");
        let secret_key: SecretKey = SecretKey::from_bytes(main_sk_array)?;
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        let pod_key: SecretKey = main_sk.derive_key(&index(self.get_num_derived_keys())).into();
        let pod_pubkey: PublicKey = pod_key.clone().public_key();
        self.pods.insert(pod_pubkey.to_bytes().to_vec(), pod_key.clone().to_bytes().to_vec());
        Ok(pod_key.to_hex().to_string())
    }

    #[instrument]
    pub fn get_num_derived_keys(&self) -> u64 {
        debug!("Number of derived keys: {}", self.pods.len());
        self.pods.len() as u64
    }

}

#[instrument]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_store_from_mnemonic() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string();
        let key_store = KeyStore::from_mnemonic(mnemonic.clone()).unwrap();

        assert_eq!(key_store.get_seed_phrase(), mnemonic);
        assert!(key_store.pods.len() > 0);
    }

    #[test]
    fn test_key_store_to_and_from_file() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string();
        let key_store = KeyStore::from_mnemonic(mnemonic).unwrap();

        let password = "test_password";
        let mut file = std::io::Cursor::new(Vec::new());

        key_store.to_file(&mut file, password).unwrap();
        file.set_position(0);

        let loaded_key_store = KeyStore::from_file(&mut file, password).unwrap();

        assert_eq!(key_store.get_seed_phrase(), loaded_key_store.get_seed_phrase());
        assert_eq!(key_store.pods.len(), loaded_key_store.pods.len());
    }

}
