use std::fs;
use std::io::Read;
use std::io::Write;
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use sha1::{Digest, Sha1};
use crate::paths;

pub fn read_object(object_hash: &str) -> anyhow::Result<Vec<u8>> {
    let (_, path) = paths::make_object_path(object_hash);

    // Read the contents of the blob object file from the .git/objects directory
    let blob_content = fs::read(path)?;

    // Decompress the contents
    let mut decoder = ZlibDecoder::new(&*blob_content);
    let mut decoded_content: Vec<u8> = Vec::new();
    decoder.read_to_end(&mut decoded_content)?;

    // Extract the actual "content" from the decompressed data
    let mut delimiter_index = 0; // = decoded_content.find('\0').unwrap();
    while decoded_content[delimiter_index] != b'\0' {
        delimiter_index += 1;
    }

    decoded_content.drain(0..delimiter_index);

    Ok(decoded_content)
}

pub fn write_object(contents: Vec<u8>, object_type: &str) -> anyhow::Result<(Vec<u8>, String)> {
    let mut object_content = Vec::new();
    object_content.write(object_type.as_bytes())?;
    object_content.write(b" ")?;
    object_content.write(format!("{}", contents.len()).as_bytes())?;
    object_content.write(b"\0")?;
    object_content.write(&contents)?;

    // Calculate sha1 of the blob content
    let raw_sha: Vec<u8> = Sha1::digest(&object_content).into_iter().collect();
    let object_hash: String = raw_sha.clone().into_iter().map(|x| format!("{:02x}", x)).collect();

    // Create a "partition" directory
    let (partition_path, object_path) = paths::make_object_path(&object_hash);
    fs::create_dir_all(partition_path)?;

    // Compress the blob using zlib
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&*object_content)?;
    let compressed_blob_content = encoder.finish()?;

    // Write the blob content to the file
    let mut object_file = fs::File::create(object_path)?;
    object_file.write(&*compressed_blob_content)?;
    object_file.sync_all()?;

    Ok((raw_sha, object_hash))
}