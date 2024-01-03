use dotenv::dotenv;

use subfile_exchange::{
    config::{Cli, Role},
    publisher::SubfilePublisher,
    subfile::ipfs::IpfsClient,
    subfile_client::SubfileDownloader,
};

#[tokio::main]
async fn main() {
    dotenv().ok();
    let cli: Cli = Cli::args();

    tracing::info!(cli = tracing::field::debug(&cli), "Running cli");

    let client = if let Ok(client) = IpfsClient::new(&cli.ipfs_gateway) {
        client
    } else {
        IpfsClient::localhost()
    };

    match cli.role {
        Role::Downloader(config) => {
            tracing::info!(
                config = tracing::field::debug(&config),
                "Downloader request"
            );
            // Create client
            let downloader = SubfileDownloader::new(client, config).await;

            // Send range request
            match downloader.download_subfile().await {
                Ok(res) => {
                    tracing::info!("Download result: {:#?}", res);
                }
                Err(e) => {
                    tracing::error!(err = e.to_string());
                }
            }
        }
        Role::Publisher(config) => {
            tracing::info!(config = tracing::field::debug(&config), "Publisher request");

            let publisher = SubfilePublisher::new(client, config);

            match publisher.publish().await {
                Ok(r) => {
                    tracing::info!(result = tracing::field::debug(&r), "Published");
                }
                Err(e) => {
                    tracing::error!(error = tracing::field::debug(&e), "Failed to build");
                }
            }
        }
        Role::Wallet(wallet_args) => {
            tracing::info!(
                server = tracing::field::debug(&wallet_args),
                "Use the provided wallet to send transactions"
            );

            // Server enable payments through the staking contract,
            // assume indexer is already registered on the staking registry contract
            //1. `allocate` - indexer address, Qm hash in bytes32, token amount, allocation_id, metadata: utils.hexlify(Array(32).fill(0)), allocation_id_proof
            //2. `close_allocate` -allocationID: String, poi: BytesLike (0x0 32bytes)
            //3. `close_allocate` and then `allocate`
            // receipt validation and storage is handled by the indexer-service framework
            // receipt redemption is handled by indexer-agent

            // Client payments - assume client signer is valid (should work without gateways)
            //1. `deposit` - to a sender address and an amount
            //2. `depositMany` - to Vec<sender address, an amount>
        }
    }
}
