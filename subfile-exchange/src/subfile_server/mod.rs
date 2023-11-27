// #![cfg(feature = "acceptor")]
use anyhow::anyhow;
use http::header::CONTENT_RANGE;
use hyper::service::{make_service_fn, service_fn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::{validate_subfile_entries, ServerArgs};
use crate::file_hasher::verify_chunk;
use crate::file_reader::read_chunk;
use crate::ipfs::IpfsClient;
use crate::subfile_reader::read_subfile;
use crate::subfile_server::util::{package_version, public_key};
use crate::types::{Health, Operator, Subfile};
// #![cfg(feature = "acceptor")]
// use hyper_rustls::TlsAcceptor;
use hyper::{Body, Request, Response, StatusCode};

use self::range::{parse_range_header, serve_file, serve_file_range};
use self::util::PackageVersion;

pub mod range;
pub mod util;

// Define a struct for the server state
#[derive(Debug)]
pub struct ServerState {
    pub operator_public_key: String,
    pub subfiles: HashMap<String, Subfile>, // Keyed by IPFS hash
    pub release: PackageVersion,
    pub free_query_auth_token: Option<String>, // Add bearer prefix
}

pub type ServerContext = Arc<Mutex<ServerState>>;

pub async fn init_server(client: &IpfsClient, config: ServerArgs) {
    let port = config.port;
    let addr = format!("{}:{}", config.host, port)
        .parse()
        .expect("Invalid address");

    let state = initialize_subfile_server_context(client, config)
        .await
        .expect("Failed to initiate subfile server");

    // Create hyper server routes
    let make_svc = make_service_fn(|_| {
        let state = state.clone();
        async { Ok::<_, hyper::Error>(service_fn(move |req| handle_request(req, state.clone()))) }
    });

    // TODO: add these to configs
    // let certs = load_certs("path/to/cert.pem").expect("Failed to load certs");
    // let key = load_private_key("path/to/key.pem").expect("Failed to load private key");

    // let tls_cfg = {
    //     let mut cfg = rustls::ServerConfig::new(rustls::NoClientAuth::new());
    //     cfg.set_single_cert(certs, key).expect("Invalid key or certificate");
    //     Arc::new(cfg)
    // };

    // let acceptor = TlsAcceptor::from(tls_cfg);
    // let server = Server::builder(hyper::server::accept::from_stream(acceptor.accept_stream()))
    //     .serve(make_svc);
    let server = hyper::server::Server::bind(&addr).serve(make_svc);

    tracing::info!("Server listening on https://{}", addr);

    if let Err(e) = server.await {
        tracing::error!("server error: {}", e);
    }
}

/// Function to initialize the subfile server
async fn initialize_subfile_server_context(
    client: &IpfsClient,
    config: ServerArgs,
) -> Result<ServerContext, anyhow::Error> {
    tracing::debug!(
        config = tracing::field::debug(&config),
        "Initializing server context"
    );

    let subfile_entries = validate_subfile_entries(config.subfiles.clone())?;
    tracing::debug!(
        entries = tracing::field::debug(&subfile_entries),
        "Validated subfile entries"
    );

    let free_query_auth_token = config
        .free_query_auth_token
        .map(|token| format!("Bearer {}", token));

    // Add the file to the service availability endpoint
    // This would be part of your server state initialization
    let mut server_state = ServerState {
        subfiles: HashMap::new(),
        release: package_version()?,
        free_query_auth_token,
        operator_public_key: public_key(&config.mnemonic)
            .expect("Failed to initiate with operator wallet"),
    };

    // Fetch the file using IPFS client
    for (ipfs_hash, local_path) in subfile_entries {
        let subfile = read_subfile(client, &ipfs_hash, local_path).await?;
        tracing::debug!(
            subfile = tracing::field::debug(&subfile),
            "Read and verify subfile"
        );

        //TODO: Refactor
        // Read all files in subfile to verify locally. This may cause a long initialization time
        for chunk_file in &subfile.chunk_files {
            // read file by chunk_file.file_name
            let mut file_path = subfile.local_path.clone();
            file_path.push(chunk_file.file_name.clone());
            tracing::trace!(file_path = tracing::field::debug(&file_path), "Verify file");

            // loop through chunk file  byte range
            for i in 0..(chunk_file.total_bytes / chunk_file.chunk_size + 1) {
                // read range
                let start = i * chunk_file.chunk_size;
                let end = u64::min(start + chunk_file.chunk_size, chunk_file.total_bytes) - 1;
                tracing::trace!(
                    i,
                    start_byte = tracing::field::debug(&start),
                    end_byte = tracing::field::debug(&end),
                    "Verify chunk index"
                );
                let chunk_hash = chunk_file.chunk_hashes[i as usize].clone();

                // read chunk
                let chunk_data = read_chunk(&file_path, (start, end))?;
                // verify chunk
                if !verify_chunk(&chunk_data, &chunk_hash) {
                    tracing::error!(
                        file = tracing::field::debug(&file_path),
                        chunk_index = tracing::field::debug(&i),
                        chunk_hash = tracing::field::debug(&chunk_hash),
                        "Cannot locally verify the serving file"
                    );
                    panic!("Local verification failed")
                }
            }
        }

        tracing::info!("Successfully verified the local serving files");

        server_state
            .subfiles
            .insert(subfile.ipfs_hash.clone(), subfile);
    }

    // Return the server state wrapped in an Arc for thread safety
    Ok(Arc::new(Mutex::new(server_state)))
}

/// Handle incoming requests by
pub async fn handle_request(
    req: Request<Body>,
    context: ServerContext,
) -> Result<Response<Body>, anyhow::Error> {
    tracing::trace!("Received request");
    match req.uri().path() {
        "/" => Ok(Response::builder()
            .status(StatusCode::OK)
            .body("Ready to roll!".into())
            .unwrap()),
        "/operator" => operator_info(&context).await,
        "/status" => status(&context).await,
        "/health" => health().await,
        "/version" => version(&context).await,
        path if path.starts_with("/subfiles/id/") => file_service(path, &req, &context).await,
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("Route not found".into())
            .unwrap()),
    }
}

/// Endpoint for server health
pub async fn health() -> Result<Response<Body>, anyhow::Error> {
    let health = Health { healthy: true };
    let health_json = serde_json::to_string(&health).map_err(|e| anyhow!(e.to_string()))?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(health_json))
        .unwrap())
}

/// Endpoint for package version
pub async fn version(context: &ServerContext) -> Result<Response<Body>, anyhow::Error> {
    let version = context.lock().await.release.version.clone();
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(version))
        .unwrap())
}

/// Endpoint for status availability
pub async fn status(context: &ServerContext) -> Result<Response<Body>, anyhow::Error> {
    let subfile_mapping = context.lock().await.subfiles.clone();
    // TODO: check for local access

    let subfile_ipfses: Vec<String> = subfile_mapping
        .keys()
        .map(|i| i.to_owned())
        .collect::<Vec<String>>();
    let json = serde_json::to_string(&subfile_ipfses).map_err(|e| anyhow!(e.to_string()))?;

    tracing::debug!(json, "Serving status");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(json))
        .unwrap())
}

// Define a handler function for the `/info` route
pub async fn operator_info(context: &ServerContext) -> Result<Response<Body>, anyhow::Error> {
    let public_key = context.lock().await.operator_public_key.clone();
    let operator = Operator { public_key };
    let json = serde_json::to_string(&operator).map_err(|e| anyhow!(e.to_string()))?;
    tracing::debug!(json, "Operator info response");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(json))
        .unwrap())
}

// Serve file requests
pub async fn file_service(
    path: &str,
    req: &Request<Body>,
    context: &ServerContext,
) -> Result<Response<Body>, anyhow::Error> {
    tracing::debug!("Received file range request");
    let id = path.trim_start_matches("/subfiles/id/");

    let context_ref = context.lock().await;
    tracing::debug!(
        subfiles = tracing::field::debug(&context_ref),
        id,
        "Received file range request"
    );

    // Validate the auth token
    let auth_token = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|t| t.to_str().ok());

    let free = context_ref.free_query_auth_token.is_none()
        || (auth_token.is_some()
            && context_ref.free_query_auth_token.is_some()
            && auth_token.unwrap() == context_ref.free_query_auth_token.as_deref().unwrap());

    if !free {
        tracing::warn!("Respond with unauthorized query");
        return Ok(Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body("Paid service is not implemented, need free query authentication".into())
            .unwrap());
    }

    let requested_subfile = match context_ref.subfiles.get(id) {
        Some(s) => s,
        None => {
            tracing::debug!(
                server_context = tracing::field::debug(&context_ref),
                id,
                "Requested subfile is not served locally"
            );
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body("Subfile not found".into())
                .unwrap());
        }
    };

    match req.headers().get("file_name") {
        Some(hash) if hash.to_str().is_ok() => {
            let mut file_path = requested_subfile.local_path.clone();
            file_path.push(hash.to_str().unwrap());
            // Parse the range header to get the start and end bytes
            match req.headers().get(CONTENT_RANGE) {
                Some(r) => {
                    tracing::debug!("Parse content range header");
                    let range = parse_range_header(r)
                        .map_err(|e| anyhow!(format!("Failed to parse range header: {}", e)))?;
                    //TODO: validate receipt
                    tracing::info!("Serve file range");
                    serve_file_range(&file_path, range).await
                }
                None => {
                    tracing::info!("Serve file");
                    serve_file(&file_path).await
                }
            }
        }
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_ACCEPTABLE)
            .body("Missing required chunk_file_hash header".into())
            .unwrap()),
    }
}