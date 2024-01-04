use ethers::contract::{abigen, Contract};
use ethers::middleware::SignerMiddleware;
use ethers::providers::{Http, Provider};
use ethers::signers::coins_bip39::English;
use ethers::signers::{Wallet, Signer};
use ethers_core::types::{H160, U256, H256, Address, Bytes};
use ethers_core::utils::{keccak256, hash_message};
use ethers_core::{k256::ecdsa::SigningKey, utils::to_checksum};
use hdwallet::{DefaultKeyChain, ExtendedPrivKey};

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use crate::errors::Error;
use crate::transaction_manager::coins_bip39::Mnemonic;
use crate::util::{derive_key_pair, UDecimal18, build_wallet};

pub type NetworkContracts =
    HashMap<String, Contract<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>>;
pub type ContractAddresses = HashMap<String, H160>;
pub type ContractClient = SignerMiddleware<Provider<Http>, Wallet<SigningKey>>;

abigen!(
    L2Staking,
    "abis/L2Staking.json",
    // "npm:@graphprotocol/contracts@latest/dist/abis/L2Staking.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

/// Test function to simply call a read fn of a contract
pub async fn controller(client: &ContractClient, contract_addr: H160) -> Result<H160, Error> {
    let contract = L2Staking::new(contract_addr, Arc::new(client.clone()));

    let value = contract
        .controller()
        .call()
        .await
        .map_err(|e| Error::ContractError(e.to_string()))?;

    Ok(value)
}

/// call staking contract allocate function
// pub async fn allocate(client: &ContractClient, contract_addr: H160, deployment_hash: String) -> Result<(), Error> {
pub async fn allocate(client: &ContractClient, contract_addr: H160) -> Result<H160, Error> {
    tracing::info!("in allocate");
    let contract = L2Staking::new(contract_addr, Arc::new(client.clone())); 
    let value = controller(client, contract_addr).await;
    tracing::info!(value = tracing::field::debug(&value), "controller helper");
    println!("allocate value");
    
    //TODO: Start with hardcoding, later add field indexer address to TX manager, tokens to fn params
    let mnemonic =
        "culture alcohol unfair success pupil economy stomach dignity beyond absurd client latin";
    let epoch: u64 = 1016;
    let deployment = "QmeaPp764FjQjPB66M9ijmQKmLhwBpHQhA7dEbH2FA1j3v";
    let existing_ids: Vec<H160> = vec![];

    let (allocation_signer, allocation_id) = unique_allocation_id(mnemonic, epoch, deployment, &existing_ids)?;
    println!("allocation signer and id {:#?}, {:#?}", allocation_signer, allocation_id);
    let decoded_bytes = bs58::decode(deployment)
        .into_vec()
        .map_err(|e| Error::InvalidConfig(format!("Failed to decode Qm Hash to bytes: {}", e.to_string())))?;
    // Truncate or pad the decoded bytes to fit into 32 bytes
    let mut deployment_byte32 = [0u8; 32];
    let len = std::cmp::min(decoded_bytes.len(), 32);
    deployment_byte32[..len].copy_from_slice(&decoded_bytes[..len]);

    let tokens = U256::from(1);
    let metadata: [u8; 32] = [0; 32];
println!("allocate metadata");
    let indexer_address = build_wallet(mnemonic)?.address();
    let proof = allocation_id_proof(&allocation_signer, indexer_address, allocation_id).await?;
println!("allocate proof: {:#?}", proof);
    let value = contract
        .allocate(deployment_byte32, tokens, allocation_id, metadata, proof)
        .call()
        .await;
        // .map_err(|e| Error::ContractError(e.to_string()))?;
    println!("allocate call result");
    tracing::info!(value = tracing::field::debug(&value), "allocate call value");
println!("allocate result: {:#?}", value);
    
    let value = 
        get_allocation(&client, contract_addr, allocation_id)
        .await
        .map_err(|e| Error::ContractError(e.to_string()))?;
    println!("get allocate: {:#?}", value); 
    tracing::info!(value = tracing::field::debug(&value), "allocate call value");

    Ok(allocation_id)
}


/// call staking contract allocate function
// pub async fn allocate(client: &ContractClient, contract_addr: H160, deployment_hash: String) -> Result<(), Error> {
pub async fn get_allocation(client: &ContractClient, contract_addr: H160, allocation_id: H160) -> Result<Allocation, Error> { 
    println!("contract addr: {:#?}", contract_addr);
    let contract = L2Staking::new(contract_addr, Arc::new(client.clone()));
    let value = contract
        .get_allocation(allocation_id)
        .call()
        .await
        .map_err(|e| Error::ContractError(e.to_string()))?;
    println!("get allocate: {:#?}", value); 
    tracing::info!(value = tracing::field::debug(&value), "allocate call value");

    Ok(value)
}

/// create proof for allocation id
pub async fn allocation_id_proof(
    signer: &Wallet<SigningKey>,
    indexer_address: H160,
    allocation_id: H160 ,
) -> Result<Bytes, Error> {
    // Convert addresses to their checksum format (EIP-55)
    let indexer_address = to_checksum(&Address::from(indexer_address), None);
    let allocation_id = to_checksum(&Address::from(allocation_id), None);

    // Hash the addresses using Keccak-256
    let message_hash = keccak256(format!("{}{}", indexer_address, allocation_id));

    // Sign the message hash
    // let signature = signer.sign_hash(H256::from(&hash_message(&message_hash))).map_err(|e| Error::WalletError(e))?;
  
    let signature = signer.sign_hash(hash_message(&message_hash)).map_err(|e| Error::WalletError(e))?;
  





    // // Wrap the message in Ethereum's specific message format
    // let eth_message_hash = hash_message(&message_hash);

    // Sign the message hash
    // let signature = signer.sign_hash(H256::from(eth_message_hash), true).await?;


    Ok(Bytes::from(signature.to_vec()))
}


// export const allocationIdProof = (
//   signer: Signer,
//   indexerAddress: string,
//   allocationId: string,
// ): Promise<string> => {
//   const messageHash = utils.solidityKeccak256(
//     ['address', 'address'],
//     [indexerAddress, allocationId],
//   )
//   const messageHashBytes = utils.arrayify(messageHash)
//   return signer.signMessage(messageHashBytes)
// }
// // logger.debug('Obtain a unique Allocation ID')
// // const { allocationSigner, allocationId } = uniqueAllocationID(
// //   this.network.transactionManager.wallet.mnemonic.phrase,
// //   context.currentEpoch.toNumber(),
// //   deployment,
// //   context.activeAllocations.map((allocation) => allocation.id),
// // )


// Function to find a unique allocation ID
// take wallet mnemonic and derive address
// take epoch and deployment for a child signer
// filter from existing ids to ensure uniqueness
fn unique_allocation_id(
    mnemonic: &str,
    epoch: u64,
    deployment: &str,
    existing_ids: &[H160],
) -> Result<(Wallet<SigningKey>, H160), Error> {
    let seed = Mnemonic::<English>::from_str(mnemonic)
        .map_err(|e| Error::InvalidConfig(e.to_string()))?
        .to_seed(None)
        .map_err(|e| Error::InvalidConfig(e.to_string()))?;

    let key_chain = DefaultKeyChain::new(
        ExtendedPrivKey::with_seed(&seed).map_err(|e| Error::InvalidConfig(e.to_string()))?,
    );

    for i in 0..100 {
        let (private_key, address) = derive_key_pair(&key_chain, epoch, deployment, i)?;
        if !existing_ids.contains(&address) {
            let wallet = Wallet::from_str(&private_key).map_err(Error::WalletError)?;
            return Ok((wallet, address));
        }
    }

    Err(Error::ContractError("Exhausted limit of 100 allocations at the same time (This should be removed as allocation parallelization is deprecated)".to_string()))
}

#[cfg(test)]
mod tests {
    use crate::transaction_manager::network_contract_addresses;

    use super::*;

    #[test]
    fn test_unique_allocation_id() {
        let indexer_mnemonic =
            "sheriff obscure trick beauty army fat wink legal flee leader section suit";
        let epoch: u64 = 0;
        let deployment = "QmeaPp764FjQjPB66M9ijmQKmLhwBpHQhA7dEbH2FA1j3v";
        let mut existing_ids: Vec<H160> = vec![];

        for i in 0..100 {
            let (_, allocation_id) =
                unique_allocation_id(indexer_mnemonic, epoch, deployment, &existing_ids).unwrap();
            existing_ids.push(allocation_id);
        }

        assert!(existing_ids.len() == 100);

        let uniquesness = unique_allocation_id(indexer_mnemonic, epoch, deployment, &existing_ids);
        assert!(uniquesness.is_err());
    }

    #[tokio::test]
    async fn test_allocate() {

        let indexer_mnemonic =
        "sheriff obscure trick beauty army fat wink legal flee leader section suit";
        println!("start");
        let provider = Provider::<Http>::try_from("https://arbitrum-goerli.infura.io/v3/dc1a550f824a4c6aa428a3376f983145").unwrap();
               println!("start provider");
        let wallet = build_wallet(indexer_mnemonic).expect("Mnemonic build wallet");
               println!("start wallet");
        let client = SignerMiddleware::new(provider, wallet);
       println!("start client");

        // Access contracts for the specified chain_id
        let contract_addresses =
            network_contract_addresses("../addresses.json", &"421614".to_string()).unwrap();
        println!("start contract address");

        let deployment = "QmeaPp764FjQjPB66M9ijmQKmLhwBpHQhA7dEbH2FA1j3v";
        let staking_addr = contract_addresses.get("L2Staking").unwrap(); 
       println!("start l2 sataking address");

        let res = allocate(&client, *staking_addr).await;
       println!("finish: {:#?}", res);

        assert!(res.is_ok())
    }
}
