# colonylib

A Rust library implementing the Colony metadata framework for the [Autonomi](https://autonomi.com) decentralized network. This library provides the core infrastructure for creating, managing, and searching metadata about files stored on Autonomi using a semantic RDF-based approach.

> **Note**: This is a library for developers. If you're looking for an end-user application to easily upload/download/search files from Autonomi, see [Colony](https://github.com/zettawatt/colony)(IN PROGRESS) or [Mutant](https://github.com/champii/mutant).

## Overview

### Core Concepts

**Pods** are the fundamental building blocks of colonylib. A pod consists of:
- An **Autonomi pointer** that serves as the pod's address
- A **scratchpad** containing RDF metadata about files and other pods
- **Semantic metadata** written in a standardized [RDF](https://www.w3.org/RDF/) schema

This architecture enables:
- **Rich metadata storage**: File types, sizes, names, descriptions, and custom properties
- **Knowledge graphs**: Pods can reference other pods, creating interconnected networks of metadata
- **Semantic search**: Query data using SPARQL via the integrated [oxigraph](https://docs.rs/oxigraph/latest/oxigraph/index.html) database
- **Decentralized discovery**: Traverse pod references to discover related content across the network

### Key Features

- **Deterministic key derivation**: Uses 12-word mnemonic seed phrases to reproducibly generate all pod addresses
- **Offline-first operation**: Build and search your metadata locally without network access or cryptocurrency
- **Cross-device synchronization**: Access your pods from any device using the same seed phrase
- **Network integration**: Upload/download pods to/from the Autonomi network when ready
- **Semantic interoperability**: Standardized RDF schemas enable data sharing between applications

### Scope

Colonylib focuses on **metadata management** and **semantic search**. It does not handle actual file uploads/downloads - those operations use the standard Autonomi API. Think of colonylib as a sophisticated indexing and discovery layer on top of Autonomi's storage primitives.

## Status

### Current Capabilities ✅

- **Local pod management**: Create, modify, and cache pods in platform-appropriate data directories (Windows, Mac, Linux)
- **Secure key management**: Password-protected keystore with deterministic key derivation from mnemonic phrases
- **Network synchronization**: Upload modified pods and download updates from the Autonomi network
- **Cache management**: Populate, repair, and maintain local pod caches with automatic conflict resolution
- **RDF graph database**: Store and query semantic metadata using oxigraph with SPARQL support
- **Pod references**: Create interconnected networks of pods with configurable traversal depth
- **Semantic search**: Query pods by content, type, properties, and relationships

### Roadmap 🚧

- Improve Autonomi error handling (auto retry on certain failures, library specific errors)
- Automatic scratchpad overflow handling for large metadata collections (>4MB)
- Advanced search features (faceted search, relevance ranking by pod depth, fuzzy matching)
- Performance optimizations for large-scale pod networks (threading Autonomi fetch operations)

## Library Architecture

Colonylib is organized into four core modules that work together to provide a complete metadata management system:

### 1. KeyStore (`key.rs`)
**Purpose**: Cryptographic key management and derivation

- **Mnemonic-based**: Generate deterministic keys from 12-word seed phrases
- **Secure storage**: Password-encrypted keystore files using the Cocoon library
- **Key derivation**: Separate key spaces for pointers, scratchpads, and wallet operations
- **Cross-device sync**: Same mnemonic produces identical keys across devices

### 2. DataStore (`data.rs`)
**Purpose**: Local file system operations and pod caching

- **Platform-aware**: Uses OS-appropriate data directories (`~/.local/share/colony` on Linux)
- **Organized storage**: Separate directories for pointers, scratchpads, and pod references
- **Cache management**: Track upload queues, handle file operations, manage local state
- **Atomic operations**: Safe concurrent access to pod files

### 3. Graph (`graph.rs`)
**Purpose**: RDF semantic database and SPARQL query engine

- **Oxigraph integration**: High-performance RDF store with SPARQL 1.1 support
- **Ontology independent**: Library supports any ontology or schema (Using [schema.org](https://schema.org/) is preferred for portability)
- **Named graphs**: Isolate pod data while enabling cross-pod queries
- **JSON-LD metadata entry**: Write JSON-LD metadata for subjects within pods
- **Query interface**: Execute SPARQL queries across all local and referenced pods and return results in JSON format

### 4. PodManager (`pod.rs`)
**Purpose**: High-level pod operations and network coordination

- **Unified interface**: Coordinates between KeyStore, DataStore, Graph, and Autonomi network
- **Pod lifecycle**: Create, modify, upload, download, and synchronize pods
- **Reference traversal**: Discover and cache interconnected pod networks
- **Search operations**: Execute semantic queries across local and referenced pods

## Public API

The main entry point for colonylib is the `PodManager` struct, which provides a high-level interface for all pod operations. Here are the key methods:

### Core Pod Operations

```rust
// Create a new pod with a given name
async fn add_pod(&mut self, pod_name: &str) -> Result<(String, String), Error>

// Create a reference from one pod to another
fn add_pod_ref(&mut self, pod_address: &str, referenced_pod_address: &str) -> Result<(), Error>

// Add metadata for a specific subject (file/resource) to a pod using JSON-LD syntax
async fn put_subject_data(&mut self, pod_address: &str, subject_address: &str, metadata: &str) -> Result<(), Error>

// Get metadata for a specific subject and return a JSON string
async fn get_subject_data(&mut self, subject_address: &str) -> Result<String, Error>
```

### Network Synchronization

```rust
// Upload all local changes to the Autonomi network
async fn upload_all(&mut self) -> Result<(), Error>

// Download updates for user-created pods
async fn refresh_cache(&mut self) -> Result<(), Error>

// Download referenced pods up to specified depth
async fn refresh_ref(&mut self, depth: u64) -> Result<(), Error>
```

### Search and Query

```rust
// Search pods using various criteria (text, type, properties)
async fn search(&mut self, query: serde_json::Value) -> Result<serde_json::Value, Error>
```

### Initialization

```rust
// Create a new PodManager instance
async fn new(
    client: Client,           // Autonomi network client
    wallet: &Wallet,          // Payment wallet
    data_store: &mut DataStore,   // Local storage
    key_store: &mut KeyStore,     // Cryptographic keys
    graph: &mut Graph,            // RDF database
) -> Result<PodManager, Error>
```

## Installation

Add colonylib to your Rust project:

```toml
[dependencies]
colonylib = "0.1.0"
autonomi = "0.4.6"
tokio = "1.44"
serde_json = "1.0"
```

Or use cargo:

```bash
cargo add colonylib autonomi tokio serde_json
```

## Examples

The repository includes three comprehensive examples that demonstrate colonylib's capabilities. These examples are designed to be run in sequence and work on a local Autonomi testnet. Running on main or the Alpha network is possible, but requires code changes.

### Prerequisites

Before running the examples, you need:

1. **Rust toolchain** (1.70 or later)
2. **Autonomi network access**:
   - **Local testnet**: Run a local Autonomi node for development. See the [Autonomi documentation](https://autonomi.com/docs) for setup instructions.
   - **Alpha network**: Connect to the Autonomi alpha testnet (change the `init_client` function in each example)
   - **Main network**: Connect to the live Autonomi network (change the `init_client` function in each example)

3. **Wallet with tokens** (for Alpha/Main network operations, creating the local testnet will handle this for you):
   - ETH for gas fees
   - ANT tokens for storage payments

### Example 1: Setup (`examples/setup.rs`)

**Purpose**: Initialize the colonylib environment and verify network connectivity.

This example:
- Creates the local data directory structure
- Initializes or loads an encrypted keystore
- Sets up the RDF graph database
- Connects to the Autonomi network
- Displays wallet balances

**Run it:**
```bash
# For local testnet
cargo run --example setup

# The example uses these default settings:
# - Network: local testnet
# - Mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
# - Password: "password"
```

**What it does:**
- Creates `~/.local/share/colony/` (Linux) or equivalent on other platforms
- Generates deterministic keys from the mnemonic phrase
- Verifies network connectivity and wallet balance
- Prepares the environment for pod operations

### Example 2: Adding Pods (`examples/add_pods.rs`)

**Purpose**: Create pods with sample metadata and upload them to the network.

This example demonstrates:
- Creating multiple pods with descriptive names
- Adding rich JSON-LD metadata for different file types
- Uploading pods to the Autonomi network
- Handling network costs and replication delays

**Run it:**
```bash
cargo run --example add_pods
```

**What it creates:**
- **Pod 1**: Metadata for an image file (`ant_girl.png`)
- **Pod 2**: Metadata for an audio file (`BegBlag.mp3`)

Each pod contains structured metadata using Schema.org vocabularies:
```json
{
  "@context": {"schema": "http://schema.org/"},
  "@type": "schema:MediaObject",
  "@id": "ant://[file-address]",
  "schema:name": "filename.ext",
  "schema:description": "File description",
  "schema:contentSize": "2MB"
}
```

### Example 3: Search (`examples/search.rs`)

**Purpose**: Demonstrate various search capabilities across the pod network.

This example shows:
- Simple text search across all metadata
- Type-based queries (find all MediaObjects)
- Property-based queries (find items with specific attributes)
- Advanced multi-criteria searches
- Subject data retrieval

**Run it:**
```bash
cargo run --example search
```

**Search types demonstrated:**
1. **Text search**: Find pods containing specific words
2. **Type search**: Query by RDF type (e.g., MediaObject, Document)
3. **Predicate search**: Find resources with specific properties
4. **Advanced search**: Combine multiple criteria
5. **Subject retrieval**: Get complete metadata for specific resources

### Running the Examples

**Complete workflow:**
```bash
# 1. Initialize the environment
cargo run --example setup

# 2. Create sample pods with metadata
cargo run --example add_pods

# 3. Search and query the pods
cargo run --example search
```

**Network Configuration:**

To use different networks, modify the `environment` variable in each example:

```rust
// Local testnet (default)
let environment = "local".to_string();

// Alpha testnet (needs test tokens)
let environment = "alpha".to_string();

// Main network (needs real tokens)
let environment = "autonomi".to_string();
```

**Wallet Configuration:**

The examples use a hardcoded private key for local testing. For production use:

1. Generate a secure private key
2. Fund the wallet with ETH and ANT tokens
3. Update the `LOCAL_PRIVATE_KEY` constant

**Data Persistence:**

- All examples use the same data directory
- Keystores and graph databases persist between runs
- You can safely re-run examples to see updated results
- Delete the data directory and run `setup.rs` to reset the environment if needed

NOTE! This is a destructive operation. It will overwrite the local data directory and recreate it.
If you have things you want to keep, make sure you have uploaded everything to Autonomi before running this.

## Usage Patterns

### Basic Workflow

```rust
use autonomi::{Client, Wallet};
use colonylib::{PodManager, DataStore, KeyStore, Graph};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize components
    let client = Client::init_local().await?;
    let wallet = &Wallet::new_from_private_key(client.evm_network(), private_key)?;
    let data_store = &mut DataStore::create()?;

    // 2. Set up keystore
    let key_store = &mut if keystore_exists {
        KeyStore::from_file(&mut file, password)?
    } else {
        KeyStore::from_mnemonic(mnemonic)?
    };

    // 3. Initialize graph database
    let graph = &mut Graph::open(&data_store.get_graph_path())?;

    // 4. Create pod manager
    let mut pod_manager = PodManager::new(client, wallet, data_store, key_store, graph).await?;

    // 5. Create and populate pods
    let (pod_addr, _) = pod_manager.add_pod("My Collection").await?;

    let metadata = json!({
        "@context": "http://schema.org/",
        "@type": "Dataset",
        "name": "Research Data",
        "description": "Important research findings"
    });

    pod_manager.put_subject_data(&pod_addr, FILE_ADDRESS, metadata).await?;

    // 6. Upload to network
    pod_manager.upload_all().await?;

    // 7. Search and query
    let results = pod_manager.search(json!("research")).await?;
    println!("Found: {}", results);

    Ok(())
}
```

### Offline-First User Support

Colonylib supports offline usage - you can create, reference, and search pods without performing any
Autonomi network write operations:

```rust
// Create pods locally
let (pod_addr, _) = pod_manager.add_pod("Offline Pod").await?;
pod_manager.put_subject_data(&pod_addr, subject, metadata).await?;

// Search works immediately
let results = pod_manager.search(json!("offline")).await?;

// Upload when ready (requires network + tokens)
pod_manager.upload_all().await?;
```

The caveat here is that the data is only stored on your computer. There is no way to recover it if
you lose your computer. Uploading to the network is necessary to ensure data persistence, cross-device
synchronization, and the ability to share your data with others.

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup

```bash
git clone https://github.com/zettawatt/colonylib.git
cd colonylib
cargo build
cargo test
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific module tests
cargo test key_tests
cargo test pod_tests
cargo test graph_tests
```

## License

This project is licensed under the GPL-3.0-only License - see the [LICENSE](LICENSE) file for details.

## Links

- **Documentation**: [docs.rs/colonylib](https://docs.rs/colonylib)
- **Repository**: [github.com/zettawatt/colonylib](https://github.com/zettawatt/colonylib)
- **Issues**: [github.com/zettawatt/colonylib/issues](https://github.com/zettawatt/colonylib/issues)
- **Autonomi Network**: [autonomi.com](https://autonomi.com)
- **Colony App**: [github.com/zettawatt/colony](https://github.com/zettawatt/colony)
