use bytes::Bytes;
use futures::StreamExt;
use object_store::{parse_url_opts, path::Path, ObjectStore};
use reqwest::Url;
use tokio::io::{AsyncWriteExt};
use futures::stream::BoxStream;
use object_store::local::LocalFileSystem;
use object_store::parse_url;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use crate::subfile::Error;

pub fn s3_store() -> Result<(Box<dyn ObjectStore>, Path), Error> {
    let url = std::env::var("S3_URL").unwrap();
    let url = Url::parse(&url).map_err(|e| Error::InvalidConfig(e.to_string()))?;
    let access_key_id = std::env::var("AWS_ACCESS_KEY_ID").unwrap();
    let secret_key_id = std::env::var("AWS_SECRET_ACCESS_KEY").unwrap();
    let bucket = std::env::var("BUCKET").unwrap();
    let region = "ams3".to_string();

    parse_url_opts(&url, 
        vec![
            ("access_key_id", access_key_id), 
            ("secret_access_key", secret_key_id), 
            ("region", region),
            ("bucket", bucket),
            ])
        .map_err(|e| Error::InvalidConfig(e.to_string()))
}

pub async fn list() -> Result<(), Error> {
    let (object_store, prefix) = s3_store()?;
    println!("object_store {:#?} ; prefix {:#?}", object_store, prefix);

    let mut list_stream = object_store.list(None);
    // let list_stream = object_store.list(Some(&prefix));

    println!("anything?");
    while let Ok(Some(meta)) = list_stream.next().await.transpose() {
        println!("somethjng");
        println!("Name: {}, size: {}", meta.location, meta.size);
    }
    Ok(())
}


pub async fn write(bytes: Bytes) -> Result<(), Error> {
    let (object_store, prefix) = s3_store()?;
    println!("got store {:#?}, {:#?}", object_store, prefix);
    let location = Path::from("test_upload_file.txt");
    // put/put_opts/multipart_put do not have the option the pick the start byte and write a chunk from there
    // we can only collect all bytes in a file and then upload with put or multipart_put which takes care of chunk uploads automatically
    // let (_multipart_id, mut writer) = object_store
    //     .put_multipart(&location)
    //     .await
    //     .map_err(|e| Error::ObjectStoreError(e.to_string()))?;
    // println!("got store writer");
    // writer
    //     .write_all(&bytes)
    //     .await
    //     .map_err(|e| Error::ObjectStoreError(e.to_string()))?;
    // println!("wrote bytes");
    // let _ = writer.shutdown();
    let put_result = object_store
        .put(&location, bytes)
        .await
        .map_err(|e| Error::ObjectStoreError(e))?;

    println!("wrote bytes: {:#?}", put_result);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::{BufReader, Read}};

    use bytes::Buf;

    use crate::{subfile::local_file_system::*, test_util::{CHUNK_SIZE, create_random_temp_file}};
    use super::*;

    #[tokio::test]
    async fn test_list_file() {
        let res = list().await;
        println!("{:#?}", res);
        assert!(res.is_ok());
    }
    
    // #[tokio::test]
    // async fn test_write_file() {
    //     let file_size = CHUNK_SIZE * 25;
    //     let (temp_file1, temp_path1) = create_random_temp_file(file_size as usize).unwrap();

    //     let path1 = std::path::Path::new(&temp_path1);
    //     let readdir1 = path1.parent().unwrap().to_str().unwrap();
    //     let file_name1 = path1.file_name().unwrap().to_str().unwrap();

    //     let file = File::open(path1).map_err(Error::FileIOError).unwrap();
    //     let mut reader = BufReader::new(file);
    //     let mut buffer = vec![0; CHUNK_SIZE.try_into().unwrap()];
    //     let bytes_read = reader.read(&mut buffer).unwrap();
        
    //     let res = write((&buffer[..bytes_read]).copy_to_bytes(bytes_read)).await;
    //     println!("{:#?}", res);
    //     assert!(res.is_ok());

    //     let res = list().await;
    //     println!("{:#?}", res);
    //     assert!(res.is_ok());
    // }
}
