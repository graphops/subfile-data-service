use alloy_primitives::{Address, U256};
use bytes::Bytes;

use http::header::{AUTHORIZATION, CONTENT_RANGE};
use rand::seq::SliceRandom;
use reqwest::Client;
use secp256k1::SecretKey;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::Mutex;

use crate::config::DownloaderArgs;
use crate::errors::Error;
use crate::subfile::{
    file_hasher::verify_chunk, ipfs::IpfsClient, subfile_reader::read_subfile, ChunkFileMeta,
    Subfile,
};
use crate::subfile_finder::{IndexerEndpoint, SubfileFinder};
use crate::transaction_manager::TransactionManager;
use crate::util::build_wallet;

use self::signer::{ReceiptSigner, TapReceipt};

pub mod signer;

pub struct SubfileDownloader {
    http_client: reqwest::Client,
    ipfs_hash: String,
    subfile: Subfile,
    _gateway_url: Option<String>,
    static_endpoints: Vec<String>,
    output_dir: String,
    free_query_auth_token: Option<String>,
    indexer_urls: Arc<StdMutex<Vec<IndexerEndpoint>>>,
    indexer_blocklist: Arc<StdMutex<HashSet<String>>>,
    // key is the chunk file identifier (IPFS hash) and value is a HashSet of downloaded chunk indices
    target_chunks: Arc<StdMutex<HashMap<String, HashSet<u64>>>>,
    chunk_max_retry: u64,
    subfile_finder: SubfileFinder,
    #[allow(dead_code)]
    receipt_signer: ReceiptSigner,
}

impl SubfileDownloader {
    pub async fn new(ipfs_client: IpfsClient, args: DownloaderArgs) -> Self {
        let subfile = read_subfile(
            &ipfs_client,
            &args.ipfs_hash,
            args.output_dir.clone().into(),
        )
        .await
        .expect("Read subfile");

        let wallet = build_wallet(&args.mnemonic).expect("Mnemonic build wallet");
        //TODO: Factor away from client, Transactions could be a separate entity
        let transaction_manager = TransactionManager::new(&args.provider, wallet.clone()).await;
        tracing::info!(
            transaction_manager = tracing::field::debug(&transaction_manager),
            "transaction_manager"
        );
        let signing_key = wallet.signer().to_bytes();
        let secp256k1_private_key =
            SecretKey::from_slice(&signing_key).expect("Private key from wallet");
        let receipt_signer = ReceiptSigner::new(
            secp256k1_private_key,
            U256::from(args.chain_id),
            Address::from_str(&args.verifier).expect("Parse verifier"),
        )
        .await;

        SubfileDownloader {
            http_client: reqwest::Client::new(),
            ipfs_hash: args.ipfs_hash,
            subfile,
            _gateway_url: args.gateway_url,
            static_endpoints: args.indexer_endpoints,
            output_dir: args.output_dir,
            free_query_auth_token: args.free_query_auth_token,
            indexer_urls: Arc::new(StdMutex::new(Vec::new())),
            indexer_blocklist: Arc::new(StdMutex::new(HashSet::new())),
            target_chunks: Arc::new(StdMutex::new(HashMap::new())),
            chunk_max_retry: args.max_retry,
            subfile_finder: SubfileFinder::new(ipfs_client),
            receipt_signer,
        }
    }

    pub fn update_indexer_urls(&self, endpoints: &[IndexerEndpoint]) {
        self.indexer_urls.lock().unwrap().clear();
        self.indexer_urls
            .lock()
            .unwrap()
            .extend(endpoints.to_owned());
    }

    pub fn add_to_indexer_blocklist(&self, endpoint: String) {
        let mut blocklist = self.indexer_blocklist.lock().expect("Failed to lock mutex");
        blocklist.insert(endpoint);
    }

    /// Read manifest to prepare chunks download
    pub fn target_chunks(&self, subfile: &Subfile) {
        for chunk_file_meta in &subfile.chunk_files {
            let mut target_chunks = self.target_chunks.lock().unwrap();
            let chunks_set = target_chunks
                .entry(chunk_file_meta.meta_info.hash.clone())
                .or_default();
            let chunk_size = chunk_file_meta.chunk_file.chunk_size;
            for i in 0..(chunk_file_meta.chunk_file.total_bytes / chunk_size + 1) {
                chunks_set.insert(i);
            }
        }
    }

    /// Read subfile manifiest and download the individual chunk files
    //TODO: update once there is payment
    pub async fn download_subfile(&self) -> Result<(), Error> {
        self.target_chunks(&self.subfile);
        tracing::info!(
            chunks = tracing::field::debug(self.target_chunks.clone()),
            "Chunk files download starting"
        );

        // check subfile availability from gateway/indexer_endpoints
        let _ = self.availbility_check().await;

        // Loop through chunk files for downloading
        let mut incomplete_files = vec![];
        for chunk_file in &self.subfile.chunk_files {
            if let Err(e) = self.download_chunk_file(chunk_file.clone()).await {
                incomplete_files.push(e);
            }
        }

        //TODO: retry for failed subfiles
        if !incomplete_files.is_empty() {
            let msg = format!(
                "Chunk files download incomplete: {:#?}",
                tracing::field::debug(&incomplete_files),
            );
            tracing::warn!(msg);
            return Err(Error::DataUnavilable(msg));
        } else {
            tracing::info!("Chunk files download completed");
        }
        Ok(())
    }

    /// Get the remaining chunks to download for a file
    pub fn remaining_chunks(&self, chunk_file_hash: &String) -> Vec<u64> {
        self.target_chunks
            .lock()
            .unwrap()
            .get(chunk_file_hash)
            .map(|chunks| chunks.clone().into_iter().collect())
            .unwrap_or_default()
    }

    /// Download a file by reading its chunk manifest
    //TODO: update once there is payment
    pub async fn download_chunk_file(&self, meta: ChunkFileMeta) -> Result<(), Error> {
        tracing::debug!(
            file_spec = tracing::field::debug(&meta),
            "Download chunk file"
        );

        // Open the output file
        let file = File::create(Path::new(
            &(self.output_dir.clone() + "/" + &meta.meta_info.name),
        ))
        .unwrap();
        let file = Arc::new(Mutex::new(file));

        while !self.remaining_chunks(&meta.meta_info.hash).is_empty() {
            // Wait for all chunk tasks to complete and collect the results
            let mut handles = Vec::new();
            for i in self.remaining_chunks(&meta.meta_info.hash) {
                let chunk_file_hash = meta.meta_info.hash.clone();
                let client = self.http_client.clone();
                //TODO: can utilize operator address for on-chain checks
                let request = self.download_range_request(&meta, i, file.clone())?;
                let block_list = self.indexer_blocklist.clone();
                let target_chunks = self.target_chunks.clone();
                let url = request.query_endpoint.clone();
                // Spawn a new asynchronous task for each range request
                let handle: tokio::task::JoinHandle<Result<Arc<Mutex<File>>, Error>> =
                    tokio::spawn(async move {
                        match download_chunk_and_write_to_file(&client, request).await {
                            Ok(r) => {
                                // Update downloaded status
                                target_chunks
                                    .lock()
                                    .unwrap()
                                    .entry(chunk_file_hash)
                                    .or_default()
                                    .remove(&i);
                                tracing::trace!(i, "Chunk downloaded");
                                Ok(r)
                            }
                            Err(e) => {
                                // If the download fails, add the URL to the indexer_blocklist
                                let url = match extract_base_url(&url) {
                                    Some(url) => url.to_string(),
                                    None => url,
                                };
                                tracing::warn!(
                                    err = e.to_string(),
                                    "Chunk file download incomplete"
                                );
                                block_list
                                    .lock()
                                    .expect("Cannot access blocklist")
                                    .insert(url);
                                Err(e)
                            }
                        }
                    });

                handles.push(handle);
            }

            for handle in handles {
                let _ = handle
                    .await
                    .map_err(|e| Error::DataUnavilable(e.to_string()))?;
            }
        }

        tracing::info!(
            chunks = tracing::field::debug(self.target_chunks.clone()),
            file_info = tracing::field::debug(&meta.meta_info),
            "File finished"
        );
        Ok(())
    }

    /// Generate a request to download a chunk
    fn download_range_request(
        &self,
        meta: &ChunkFileMeta,
        i: u64,
        file: Arc<Mutex<File>>,
    ) -> Result<DownloadRangeRequest, Error> {
        let mut rng = rand::thread_rng();
        let query_endpoints = &self.indexer_urls.lock().unwrap();
        let url = if let Some((operator, url)) = query_endpoints.choose(&mut rng).cloned() {
            tracing::debug!(
                operator,
                url,
                chunk = i,
                chunk_file = meta.meta_info.hash,
                "Querying operator"
            );
            url.clone()
        } else {
            let err_msg = "No operator serving the file, data unavailable".to_string();
            tracing::warn!(err_msg);
            return Err(Error::DataUnavilable(err_msg.to_string()));
        };
        //TODO: do no add ipfs_hash here, construct query_endpoint after updating route 'subfiles/id/:id'
        let query_endpoint = url + "/subfiles/id/" + &self.ipfs_hash;
        let file_hash = meta.meta_info.hash.clone();
        let start = i * meta.chunk_file.chunk_size;
        let end = u64::min(
            start + meta.chunk_file.chunk_size,
            meta.chunk_file.total_bytes,
        ) - 1;
        let chunk_hash = meta.chunk_file.chunk_hashes[i as usize].clone();

        Ok(DownloadRangeRequest {
            query_endpoint,
            file_hash,
            start,
            end,
            chunk_hash,
            file,
            max_retry: self.chunk_max_retry,
            auth_token: self.free_query_auth_token.clone(),
            _receipt: None,
        })
    }

    async fn availbility_check(&self) -> Result<(), Error> {
        let blocklist = self.indexer_blocklist.lock().unwrap().clone();
        let endpoints = &self
            .static_endpoints
            .iter()
            .filter(|url| !blocklist.contains(*url))
            .cloned()
            .collect::<Vec<_>>();
        self.update_indexer_urls(
            &self
                .subfile_finder
                .subfile_availabilities(&self.ipfs_hash, endpoints)
                .await,
        );
        let indexer_endpoints = self.indexer_urls.lock().unwrap().clone();
        if indexer_endpoints.is_empty() {
            tracing::warn!(
                subfile_hash = &self.ipfs_hash,
                "No endpoint satisfy the subfile requested, sieve through available subfiles for individual files"
            );

            // check files availability from gateway/indexer_endpoints
            match self
                .subfile_finder
                .file_discovery(&self.ipfs_hash, endpoints)
                .await
            {
                Ok(map) => {
                    let msg = format!(
                        "Files available on these available subfiles: {:#?}",
                        tracing::field::debug(&map.lock().await),
                    );
                    return Err(Error::DataUnavilable(msg));
                }
                Err(e) => {
                    let msg = format!(
                        "Cannot match the files: {:?}, {:?}",
                        tracing::field::debug(&self.indexer_urls.lock().unwrap()),
                        tracing::field::debug(&e),
                    );
                    tracing::error!(msg);
                    return Err(Error::DataUnavilable(msg));
                }
            }
        };
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DownloadRangeRequest {
    query_endpoint: String,
    auth_token: Option<String>,
    _receipt: Option<TapReceipt>,
    file_hash: String,
    start: u64,
    end: u64,
    chunk_hash: String,
    file: Arc<Mutex<File>>,
    max_retry: u64,
}

/// Make request to download a chunk and write it to the file in position
async fn download_chunk_and_write_to_file(
    http_client: &Client,
    request: DownloadRangeRequest,
) -> Result<Arc<Mutex<File>>, Error> {
    let mut attempts = 0;

    tracing::debug!(
        request = tracing::field::debug(&request),
        "Making a range request"
    );
    loop {
        // Make the range request to download the chunk
        match request_chunk(
            http_client,
            &request.query_endpoint,
            request.auth_token.clone(),
            &request.file_hash,
            request.start,
            request.end,
        )
        .await
        {
            Ok(data) => {
                if verify_chunk(&data, &request.chunk_hash) {
                    // Lock the file for writing
                    let mut file_lock = request.file.lock().await;
                    file_lock
                        .seek(SeekFrom::Start(request.start))
                        .map_err(Error::FileIOError)?;
                    file_lock.write_all(&data).map_err(Error::FileIOError)?;
                    drop(file_lock);
                    return Ok(request.file); // Successfully written the chunk, exit loop
                } else {
                    // Immediately return and blacklist the indexer when a chunk received is invalid
                    let msg = format!(
                        "Failed to validate received chunk: {}",
                        &request.query_endpoint
                    );
                    tracing::warn!(msg);
                    return Err(Error::ChunkInvalid(msg));
                }
            }
            Err(e) => tracing::error!("Chunk download error: {:?}", e),
        }

        attempts += 1;
        if attempts >= request.max_retry {
            return Err(Error::DataUnavilable(
                "Max retry attempts reached for chunk download".to_string(),
            ));
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Make range request for a file to the subfile server
async fn request_chunk(
    http_client: &Client,
    query_endpoint: &str,
    auth_token: Option<String>,
    file_hash: &str,
    start: u64,
    end: u64,
) -> Result<Bytes, Error> {
    let range = format!("bytes={}-{}", start, end);
    //TODO: implement payment flow
    // if auth_token.is_none() {
    //     tracing::error!(
    //         "No auth token provided; require receipt implementation"
    //     );
    //     Err(anyhow!("No auth token"))
    // };

    tracing::debug!(query_endpoint, range, "Make range request");
    let response = http_client
        .get(query_endpoint)
        .header("file_hash", file_hash)
        .header(CONTENT_RANGE, range)
        .header(
            AUTHORIZATION,
            auth_token.expect("No payment nor auth token"),
        )
        .send()
        .await
        .map_err(Error::Request)?;

    // Check if the server supports range requests
    if response.status().is_success() && response.headers().contains_key(CONTENT_RANGE) {
        Ok(response.bytes().await.map_err(Error::Request)?)
    } else {
        let err_msg = format!(
            "Server does not support range requests or the request failed: {:#?}",
            tracing::field::debug(&response.status()),
        );
        tracing::error!(
            status = tracing::field::debug(&response.status()),
            headers = tracing::field::debug(&response.headers()),
            chunk = tracing::field::debug(&response),
            "Server does not support range requests or the request failed"
        );
        Err(Error::InvalidRange(err_msg))
    }
}

/// extract base indexer_url from `indexer_url/subfiles/id/subfile_id`
fn extract_base_url(query_endpoint: &str) -> Option<&str> {
    if let Some(index) = query_endpoint.find("/subfiles/id/") {
        Some(&query_endpoint[..index])
    } else {
        None
    }
}
