# File Sharing Client

Tired of data drudgery? Imagine a world where you skip the tedious task of indexing data and start serving queries in a flash! That's the magic of our P2P file sharing network. Instead of spending hours to months catching up to the head of a chain, you can tap into a vast pool of indexed information, ready and waiting for your queries. 

This document provides an overview of the file sharing client.

## Functionality

The client facilitates data retrieval from the P2P file sharing network. Users can specify desired files by their content addressable IDs (CIDs) and utilize various features:

- File retrieval: Download entire files or specific chunks at a time.
- Payment: Pay for file access using tokens deposited in an Escrow account on-chain.
- Free access: Utilize a free query auth token for limited access to particular servers.
- Indexer selection: Choose from multiple indexer endpoints; Currently just for availability, later add optimization for performance and redundancy.

## Minimal Trust Requirement

To minimize trust requirements, the client employs a chunk-based payment system. Instead of paying for the entire file upfront, users can pay for individual chunks as they download and verify them. This ensures transparency and reduces the risk of losing funds due to server downtime or malicious actors.

## CLI Usage

The client operates through a command-line interface (CLI) for simplicity and ease of use. Client would need to determine the subfile that contains the dataset they desire. This may mean looking at subfile manifests or in the future a tool that matches subfiles by the provided critieria. 

After determining the subfile CID, client should supply a local path for writing the subfile corresponding files, a wallet for payments or a free query auth token, and a list of indexer endpoints (this should be handled by gateway or a scraping client).

### CLI example
```
➜  subfile-exchange git:(main) ✗ cargo run -p subfile-exchange downloader \
   --ipfs-hash QmeaPp764FjQjPB66M9ijmQKmLhwBpHQhA7dEbH2FA1j3v \
   --indexer-endpoints http://localhost:5678,http://localhost:5677 \
   --free-query-auth-token 'Bearer imfreeee' \
   --mnemonic "sheriff obscure trick beauty army fat wink legal flee leader section suit" \
   --chain-id 421614 \
   --verifier 0xfC24cE7a4428A6B89B52645243662A02BA734ECF \
   --provider [arbitrum-sepolia-rpc-endpoint]
```


### Requirements

To use the client effectively, you will need:

- Content Address: The CID of the desired file.
- Local Path: A directory where the downloaded file will be stored. (Later will be a generic storage path, enabling cloud storage access)
- Wallet: A blockchain wallet containing tokens for escrow payments.
- Indexer Endpoints: A list of available server addresses.
- Free Query Auth Token (Optional): For limited access to small files.

### Getting Started

1. Download and install the source code.
2. Gather configurations: Identify the CID of the desired subfile, registered indexer endpoints, a local path for storing the downloaded files, private key (or mnemonics) of a wallet valid for Escrow payments, (optional) Obtain a free query auth token for limited access.
3. Use the CLI commands to download files.

Enjoy seamless access to a vast world of data!

### Security Considerations

The client prioritizes user safety and security. It employs secure communication protocols and wallet management practices. However, users should always be mindful of potential risks:

- Choosing subfiles: Verify correctness of the subfile before initiating file requests.
- Securing wallet: Implement strong key protection and other security measures for the wallet.
- Staying informed: Updated on the latest security threats, invalid subfiles, and updates for the client software.

### Join the Community

To learn more, share experiences, and contribute to the network's growth.

- Discord channel to be created
- Documentation: Explore detailed guides and technical information.
- GitHub Repository: Contribute to the client's development and propose improvements.
