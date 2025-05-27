# colonylib

Rust library implementing the Colony metadata framework for the [Autonomi](https://autonomi.com) network. This library is not intended as an end user application. If you're looking for an app to easily upload/download/search files from Autonomi, see [Colony](https://github.com/zettawatt/colony).

## Overview

The core concept of colonylib is that of a 'pod'. A pod is made up of the Autonomi pointer and scratchpad primitive datatypes. A pod consists of a pointer with its target pointing to a scratchpad. The scratchpads themselves contain file metadata written in a specific [RDF](https://www.w3.org/RDF/) schema that enable describing addresses stored on the Autonomi network. Information such as the type of data stored at that address, its size, a name, etc. can all be stored in the pod's scratchpad. A pod can also describe the address of another pod, thereby enabling the creation of a knowledge graph through colonylib's network traversal algorithm. Finally, because of the consistent [RDF](https://www.w3.org/RDF/) schema and metadata conventions, the data can be searched with colonylib via SPARQL query by leveraging the [oxigraph](https://docs.rs/oxigraph/latest/oxigraph/index.html) crate.

For portability and interoperability, colonylib leverages the concept of a master key or 12 word mnemonic seed phrase to repeatably generate all pods stored on the Autonomi network. As long as a user has the same master key or phrase, access to and the contents of all pod data placed onto the Autonomi network can be recovered and utilized at any time.

All of colonylibs network read operations can be done without uploading to the Autonomi network at all by leveraging a local on disk cache for all pod operations. This means a user can add to their local list of pods, construct a local knowledge graph, search the network, and download content, all without having access to any crypto currency. At any time, the user can add ANT/ETH tokens to a wallet and upload their pods to the network, but they are not forced to. In this mode of operation, it is similar to any other file sharing application used for downloads only, making it easy for anyone to start using the end application.

The colonylib library enables creating, modifying, and searching for files on the Autonomi network using the above pod infrastructure, it does not however handle file uploads/downloads. These are handled by the standard Autonomi API.

## Status

Currently colonylib has the ability:
- Create a pod cache and place it in the proper data directory, whether Windows, Mac, or Linux
- Create a key store protected by a password to manage all created private keys
- Create and modify pods in the local on disk cache
- Upload all marked modified cached pod files to Autonomi
- Download all pods from Autonomi that are of a newer version than those in the local cache
- Populate a local cache from scratch or repair a corrupted cache with pods from Autonomi

Next steps:
- Define and implement a simple RDF ontology for files and pod references
- Add methods to add files and references to pods to local pods using the RDF ontology
- Handle pod scratchpad overflow conditions (i.e. if a scratchpad's data is more than 4MB, create a new linked scratchpad)
- Parse the local cache of pod scratchpads into the oxigraph RocksDB database
- Perform search by name and type using the oxigraph query method

## Library Description

The library currently consists of 3 parts:
1. [key.rs](./src/key.rs): A key store to create a new master key and derive keys for Autonomi pointer and scratchpad data types
2. [data.rs](./src/data.rs): Local disk and pod cache operations
3. [pod.rs](./src/pod.rs): Functions to create and modify pods on the local disk cache and to download/upload pods from the Autonomi network

## How to install colonylib

The latest versions of colonylib are pushed to crates.io. You can add this library to any rust project by calling:
```
cargo add colonylib
```
