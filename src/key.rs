use borsh::{BorshDeserialize, BorshSerialize};
use bip39::{Mnemonic, Language};
use autonomi::client::key_derivation::{DerivationIndex, MainSecretKey};
use autonomi::{SecretKey, PublicKey};
use cocoon::Cocoon;
use std::collections::HashMap;
use std::io::Error;
use sn_bls_ckd::derive_master_sk;
use sn_curv::elliptic::curves::ECScalar;
use hex;

#[derive(BorshDeserialize, BorshSerialize, Clone)] // Ensure BorshSerialize is derived
pub struct KeyStore {
    wallet_key: Vec<u8>,
    mnemonic: String,
    main_sk: Vec<u8>,
    pods: HashMap<Vec<u8>, Vec<u8>>,
}

impl KeyStore {
    pub fn from_file<R: std::io::Read>(file: &mut R, password: &str) -> Result<Self, cocoon::Error> {
        let cocoon = Cocoon::new(&password.as_bytes());
        let encoded = cocoon.parse(file)?;
        let key_store = KeyStore::try_from_slice(&encoded).unwrap();
        Ok(key_store)
    }
    pub fn to_file<W: std::io::Write>(&self, file: &mut W, password: &str) -> Result<(), cocoon::Error> {
        let mut cocoon = Cocoon::new(&password.as_bytes());
        let encoded = borsh::to_vec(&self).unwrap();
        cocoon.dump(encoded, file)?;
        Ok(())
    }
    pub fn from_mnemonic(mnemonic: String) -> Result<Self, Error> {

        // Generate a new mnemonic from the given phrase
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, mnemonic.as_str()).unwrap();
        let seed = mnemonic.to_seed_normalized("");

        // Derive BLS12-381 master secret key from seed using EIP-2333 standard.
        // Guarantees a valid, non-zero scalar represented as 32 Big-Endian bytes.
        let key_bytes: [u8; 32] = derive_master_sk(&seed)
            .expect("derive_master_sk failed; seed length requirement is >= 32 bytes")
            .serialize() // Get the 32-byte Big-Endian representation
            .into(); // Convert GenericArray<u8, 32> to [u8; 32]

        // Create a SecretKey from the 32-byte array
        let secret_key = SecretKey::from_bytes(key_bytes).unwrap_or_else(|error| {
                panic!("Problem creating the secret key. Try running initialize again: {:?}", error);
            }
        );

        // Generate a new main keys from the mnemonic
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        //let main_pk: MainPubkey = main_sk.public_key();

        // Create a new pods hashmap and add the first pod
        let mut pods: HashMap<PublicKey, SecretKey> = HashMap::new();
        let pod_key: SecretKey = main_sk.derive_key(&index(0)).into();
        let pod_pubkey: PublicKey = pod_key.public_key();
        pods.insert(pod_pubkey, pod_key.clone());

        Ok(KeyStore {
            wallet_key: SecretKey::default().to_bytes().to_vec(),
            mnemonic: mnemonic.to_string(),
            main_sk: main_sk.to_bytes(),
            pods: pods.iter().map(|(k, v)| (k.to_bytes().to_vec(), v.to_bytes().to_vec())).collect()
        })
    }

    pub fn get_seed_phrase(&self) -> String {
        self.mnemonic.clone()
    }

    pub fn set_wallet_key(&mut self, wallet_key: String) {
        let wallet_key = remove_0x_prefix(wallet_key.as_str());
        self.wallet_key = hex::decode(wallet_key).unwrap();
    }

    pub fn get_wallet_key(&self) -> String {
        hex::encode(self.wallet_key.clone())
    }

    pub fn get_pod_key(&self, pod_pubkey: String) -> String {
        let decoded_key = hex::decode(pod_pubkey.as_str()).unwrap();
        match self.pods.get(&decoded_key) {
            Some(value) => hex::encode(value),
            None => String::from("Key not found"),
        }
    }

    pub fn add_derived_key(&mut self) -> String {
        let main_sk_array: [u8; 32] = self.main_sk.clone().try_into().expect("main_sk must be 32 bytes");
        let secret_key: SecretKey = SecretKey::from_bytes(main_sk_array).unwrap();
        let main_sk: MainSecretKey = MainSecretKey::new(secret_key);
        let pod_key: SecretKey = main_sk.derive_key(&index(self.get_num_derived_keys())).into();
        let pod_pubkey: PublicKey = pod_key.clone().public_key();
        self.pods.insert(pod_pubkey.to_bytes().to_vec(), pod_key.clone().to_bytes().to_vec());
        pod_key.to_hex().to_string()
    }

    pub fn get_num_derived_keys(&self) -> u64 {
        self.pods.len() as u64
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
