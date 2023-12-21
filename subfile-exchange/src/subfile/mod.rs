pub mod file_hasher;
pub mod file_reader;
pub mod ipfs;
pub mod local_file_system;
pub mod remote_object_store;
pub mod subfile_reader;

use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use crate::{
    errors::Error,
    subfile::{
        file_hasher::{hash_chunk, verify_chunk},
        file_reader::{chunk_file, format_path, read_chunk},
        ipfs::is_valid_ipfs_hash,
    },
};

/* Public Manifests */

/// Better mapping of files and chunk files
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SubfileManifest {
    pub files: Vec<FileMetaInfo>,
    pub file_type: String,
    pub spec_version: String,
    pub description: String,
    pub chain_id: String,
    pub block_range: BlockRange,
    // pub identifier: String,
    // pub publisher_url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct FileMetaInfo {
    pub name: String,
    pub hash: String,
    // Some tags for discovery and categorization
    // pub block_range: BlockRange,
}

/* Chunk file */
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct ChunkFile {
    pub total_bytes: u64,
    pub chunk_size: u64,
    pub chunk_hashes: Vec<String>,
}

impl ChunkFile {
    pub fn new(read_dir: &str, file_name: &str, chunk_size: u64) -> Result<ChunkFile, Error> {
        let file_path = format_path(read_dir, file_name);
        // let merkle_root = hex::encode(merkle_tree.root());
        // let chunk_hashes: Vec<String> = merkle_tree.nodes().iter().map(hex::encode).collect();
        let (total_bytes, chunks) = chunk_file(Path::new(&file_path), chunk_size)?;

        let chunk_hashes: Vec<String> = chunks.iter().map(|c| hash_chunk(c)).collect();

        Ok(ChunkFile {
            total_bytes,
            chunk_size,
            chunk_hashes,
        })
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Eq, PartialEq, Clone)]
pub struct ChunkFileMeta {
    pub meta_info: FileMetaInfo,
    pub chunk_file: ChunkFile,
}

/* Subfile - packaging of chunk files mapped into local files */
//TODO: Add GraphQL derivation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Subfile {
    pub ipfs_hash: String,
    pub local_path: PathBuf,
    pub manifest: SubfileManifest,
    /// IPFS hash, Chunk file spec
    pub chunk_files: Vec<ChunkFileMeta>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockRange {
    pub start_block: Option<u64>,
    pub end_block: Option<u64>,
}

// #[derive(ValueEnum, Clone, Debug, Serialize, Deserialize, Default)]
// pub enum FileType {
//     #[default]
//     SqlSnapshot,
//     Flatfiles,
// }

// impl FromStr for FileType {
//     type Err = &'static str;

//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         match s {
//             "sql_snapshot" => Ok(FileType::SqlSnapshot),
//             "flatfiles" => Ok(FileType::Flatfiles),
//             _ => Err("Invalid file type"),
//         }
//     }
// }

impl Subfile {
    /// Validate the local files against a given subfile specification
    pub fn validate_local_subfile(&self) -> Result<&Self, Error> {
        tracing::debug!(
            subfile = tracing::field::debug(self),
            "Read and verify subfile"
        );

        // Read all files in subfile to verify locally. This may cause a long initialization time
        for file_meta in &self.chunk_files {
            self.read_and_validate_file(file_meta)?;
        }

        tracing::debug!("Successfully verified the local serving files");
        Ok(self)
    }

    /// Read and validate file
    pub fn read_and_validate_file(&self, file: &ChunkFileMeta) -> Result<(), Error> {
        // read file by chunk_file.file_name
        let meta_info = &file.meta_info;
        let chunk_file = &file.chunk_file;
        let mut file_path = self.local_path.clone();
        file_path.push(meta_info.name.clone());
        tracing::trace!(
            file_path = tracing::field::debug(&file_path),
            chunk_file = tracing::field::debug(&chunk_file),
            "Verify file"
        );

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
                return Err(Error::InvalidConfig(format!(
                    "Failed to validate the local version of file {}",
                    meta_info.hash
                )));
            }
        }
        Ok(())
    }
}

/// Validate the subfile configurations at initialization
pub fn validate_subfile_entries(entries: Vec<String>) -> Result<Vec<(String, PathBuf)>, Error> {
    let mut results = Vec::new();

    for entry in entries {
        results.push(validate_subfile_entry(entry)?);
    }

    Ok(results)
}

/// Subfile entry must be in the format of "valid_ipfs_hash:valid_local_path"
pub fn validate_subfile_entry(entry: String) -> Result<(String, PathBuf), Error> {
    let parts: Vec<&str> = entry.split(':').collect();
    if parts.len() != 2 {
        return Err(Error::InvalidConfig(format!(
            "Invalid format for entry: {}",
            entry
        )));
    }

    let ipfs_hash = parts[0];
    let local_path = parts[1];

    if !is_valid_ipfs_hash(ipfs_hash) {
        return Err(Error::InvalidConfig(format!(
            "Invalid IPFS hash: {}",
            ipfs_hash
        )));
    }

    // Validate local path
    let path = PathBuf::from_str(local_path).map_err(|e| Error::InvalidConfig(e.to_string()))?;
    if !path.exists() {
        return Err(Error::InvalidConfig(format!(
            "Path do not exist: {}",
            local_path
        )));
    }

    Ok((ipfs_hash.to_string(), path))
}

#[cfg(test)]
mod tests {
    use crate::test_util::simple_subfile;

    #[test]
    fn test_read_and_validate_file() {
        let mut subfile = simple_subfile();
        let file_meta = subfile.chunk_files.first().unwrap();
        let result = subfile.read_and_validate_file(file_meta);
        assert!(result.is_ok());

        // Add tests for failure cases
        if let Some(file_meta) = subfile.chunk_files.first_mut() {
            if let Some(first_hash) = file_meta.chunk_file.chunk_hashes.first_mut() {
                *first_hash += "1";
            }
        }
        let file_meta = subfile.chunk_files.first().unwrap();
        let result = subfile.read_and_validate_file(file_meta);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_local_subfile() {
        let mut subfile = simple_subfile();
        let result = subfile.validate_local_subfile();
        assert!(result.is_ok());

        // Add tests for failure cases
        if let Some(file_meta) = subfile.chunk_files.first_mut() {
            if let Some(first_hash) = file_meta.chunk_file.chunk_hashes.first_mut() {
                *first_hash += "1";
            }
        }
        let result = subfile.validate_local_subfile();
        assert!(result.is_err());
    }
}
