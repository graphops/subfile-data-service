use build_info::BuildInfo;

use serde::{Deserialize, Serialize};
use std::fs;
use std::{collections::HashMap, io};
use subfile_exchange::util::{build_wallet, wallet_address};

use subfile_exchange::errors::{Error, ServerError};

#[derive(Serialize, Deserialize)]
pub struct Health {
    pub healthy: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Operator {
    #[serde(alias = "publicKey")]
    pub public_key: String,
}

/// Struct for version control
#[derive(Serialize, Debug, Clone)]
pub struct PackageVersion {
    pub version: String,
    pub dependencies: HashMap<String, String>,
}

impl From<&BuildInfo> for PackageVersion {
    fn from(value: &BuildInfo) -> Self {
        Self {
            version: value.crate_info.version.to_string(),
            dependencies: HashMap::from_iter(
                value
                    .crate_info
                    .dependencies
                    .iter()
                    .map(|d| (d.name.clone(), d.version.to_string())),
            ),
        }
    }
}

// Load public certificate from file.
#[allow(unused)]
fn load_certs(filename: &str) -> Result<Vec<rustls::Certificate>, Error> {
    // Open certificate file.
    let certfile = fs::File::open(filename).map_err(|e| {
        Error::ServerError(ServerError::ContextError(format!(
            "failed to open {}: {}",
            filename, e
        )))
    })?;
    let mut reader = io::BufReader::new(certfile);

    // Load and return certificate.
    let certs = rustls_pemfile::certs(&mut reader).map_err(|e| {
        Error::ServerError(ServerError::ContextError(format!(
            "failed to load certificate: {:#?}",
            e
        )))
    })?;
    Ok(certs.into_iter().map(rustls::Certificate).collect())
}

// Load private key from file.
#[allow(unused)]
fn load_private_key(filename: &str) -> Result<rustls::PrivateKey, Error> {
    // Open keyfile.
    let keyfile = fs::File::open(filename).map_err(|e| {
        Error::ServerError(ServerError::ContextError(format!(
            "failed to open {}: {}",
            filename, e
        )))
    })?;
    let mut reader = io::BufReader::new(keyfile);

    // Load and return a single private key.
    let keys = rustls_pemfile::rsa_private_keys(&mut reader).map_err(|e| {
        Error::ServerError(ServerError::ContextError(format!(
            "failed to load private key: {:#?}",
            e
        )))
    })?;
    if keys.len() != 1 {
        return Err(Error::ServerError(ServerError::ContextError(
            "Expected a single private key".to_string(),
        )));
    }

    Ok(rustls::PrivateKey(keys[0].clone()))
}

/// Validate that private key as an Eth wallet
pub fn public_key(value: &str) -> Result<String, Error> {
    let wallet = build_wallet(value)?;
    let addr = wallet_address(&wallet);
    tracing::trace!(address = addr, "Resolved wallet address");
    Ok(addr)
}
