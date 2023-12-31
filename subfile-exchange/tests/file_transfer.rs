#[cfg(test)]
mod tests {
    use std::{process::Command, time::Duration};
    use tempfile::tempdir;
    use tokio::fs;

    use subfile_exchange::{
        config::DownloaderArgs, subfile::ipfs::IpfsClient, subfile_client::SubfileDownloader,
        test_util::server_ready,
    };

    #[tokio::test]
    async fn test_file_transfer() {
        std::env::set_var("RUST_LOG", "off,subfile_exchange=debug,file_transfer=trace");
        subfile_exchange::config::init_tracing(String::from("pretty")).unwrap();

        let client = IpfsClient::new("https://ipfs.network.thegraph.com")
            .expect("Could not create client to thegraph IPFS gateway");
        let target_subfile = "QmeaPp764FjQjPB66M9ijmQKmLhwBpHQhA7dEbH2FA1j3v".to_string();
        // 1. Setup server
        let mut server_process = Command::new("cargo")
            .arg("run")
            .arg("-p")
            .arg("subfile-service")
            .arg("--")
            .arg("--mnemonic")
            .arg("sheriff obscure trick beauty army fat wink legal flee leader section suit")
            .arg("--subfiles")
            .arg(format!("{}:./../example-file/", target_subfile))
            .spawn()
            .expect("Failed to start server");
        tracing::debug!("Wait 10 seconds");
        tokio::time::sleep(Duration::from_secs(10)).await;
        let _ = server_ready("http://0.0.0.0:5678/status").await;

        // 2. Setup downloader
        let temp_dir = tempdir().unwrap();
        let output_dir = temp_dir.path().to_path_buf();

        let downloader_args = DownloaderArgs {
            output_dir: output_dir.to_str().unwrap().to_string(),
            ipfs_hash: target_subfile,
            indexer_endpoints: [
                "http://localhost:5678".to_string(),
                "http://localhost:5677".to_string(),
            ]
            .to_vec(),
            chain_id: 421614,
            verifier: String::from("0xfC24cE7a4428A6B89B52645243662A02BA734ECF"),
            mnemonic: String::from(
                "sheriff obscure trick beauty army fat wink legal flee leader section suit",
            ),
            free_query_auth_token: Some("Bearer free-token".to_string()),
            provider: String::from("https://arbitrum-sepolia.infura.io/v3/aaaaaaaaaaaaaaaaaaaa"),
            ..Default::default()
        };

        let downloader = SubfileDownloader::new(client, downloader_args).await;

        // 3. Perform the download
        let download_result = downloader.download_subfile().await;

        // 4. Validate the download
        assert!(download_result.is_ok());
        // Further checks can be added to verify the contents of the downloaded files

        // 5. Cleanup
        fs::remove_dir_all(temp_dir).await.unwrap();
        let _ = server_process.kill();
    }
}
