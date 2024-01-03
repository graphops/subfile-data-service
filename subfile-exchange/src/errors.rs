use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum Error {
    InvalidConfig(String),
    FileIOError(std::io::Error),
    InvalidRange(String),
    IPFSError(reqwest::Error),
    SubfileError(String),
    Request(reqwest::Error),
    DataUnavilable(String),
    ChunkInvalid(String),
    ServerError(ServerError),
    JsonError(serde_json::Error),
    YamlError(serde_yaml::Error),
    InvalidPriceFormat(String),
    ContractError(String),
    ObjectStoreError(object_store::Error),
    WalletError(ethers::signers::WalletError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::InvalidConfig(ref msg) => write!(f, "Invalid configuration: {}", msg),
            Error::FileIOError(ref err) => write!(f, "File IO error: {}", err),
            Error::InvalidRange(ref msg) => write!(f, "Invalid range: {}", msg),
            Error::IPFSError(ref err) => write!(f, "IPFS error: {}", err),
            Error::SubfileError(ref msg) => write!(f, "Subfile error: {}", msg),
            Error::Request(ref err) => write!(f, "Client error: {}", err),
            Error::DataUnavilable(ref err) => write!(f, "Client error: {}", err),
            Error::ChunkInvalid(ref err) => write!(f, "Chunk invalid error: {}", err),
            Error::ServerError(ref err) => write!(f, "Server error: {}", err),
            Error::JsonError(ref err) => write!(f, "JSON error: {}", err),
            Error::YamlError(ref err) => write!(f, "YAML error: {}", err),
            Error::InvalidPriceFormat(ref msg) => write!(f, "Price format error: {}", msg),
            Error::ContractError(ref msg) => write!(f, "Contract call error: {}", msg),
            Error::ObjectStoreError(ref err) => write!(f, "Object store error: {}", err),
            Error::WalletError(ref err) => write!(f, "Wallet error: {}", err),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match *self {
            Error::FileIOError(ref e) => Some(e),
            Error::IPFSError(ref e) => Some(e),
            Error::JsonError(ref e) => Some(e),
            Error::YamlError(ref e) => Some(e),
            Error::Request(ref e) => Some(e),
            Error::ServerError(ref e) => e.source(),
            _ => None,
        }
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ServerError::ContextError(ref msg) => write!(f, "Context error: {}", msg),
            ServerError::RequestBodyError(ref msg) => write!(f, "Request body error: {}", msg),
            ServerError::HeaderParseError(ref msg) => write!(f, "Header parse error: {}", msg),
            ServerError::MethodParseError(ref msg) => write!(f, "Method parse error: {}", msg),
            ServerError::ParamsParseError(ref msg) => write!(f, "Params parse error: {}", msg),
            ServerError::BuildResponseError(ref msg) => write!(f, "Build response error: {}", msg),
        }
    }
}

impl StdError for ServerError {}

#[derive(Debug)]
pub enum ServerError {
    ContextError(String),
    RequestBodyError(String),
    HeaderParseError(String),
    MethodParseError(String),
    ParamsParseError(String),
    BuildResponseError(String),
}
