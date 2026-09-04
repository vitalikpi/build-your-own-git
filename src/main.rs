use std::env;
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use sha1::{Digest, Sha1};
use std::fs;
use std::io::Read;
use std::io::prelude::*;
use std::path;
fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    eprintln!("Logs from your program will appear here!");

    let args: Vec<String> = env::args().collect();
    if args[1] == "init" {
        fs::create_dir(".git").unwrap();
        fs::create_dir(".git/objects").unwrap();
        fs::create_dir(".git/refs").unwrap();
        fs::write(".git/HEAD", "ref: refs/heads/main\n").unwrap();
        println!("Initialized git directory");
    } else if args[1] == "cat-file" {
        let object_hash = args[3].as_str();
        cat_file(object_hash);
    } else if args[1] == "hash-object" {
        let path = &args[3];
        hash_object(path);
    } else {
        println!("unknown command: {}", args[1])
    }
}

fn cat_file(object_hash: &str) {
    let path: path::PathBuf = make_object_path(object_hash);

    // Read the contents of the blob object file from the .git/objects directory
    let blob_content = fs::read(path).unwrap();

    // Decompress the contents
    let mut decoder = ZlibDecoder::new(&*blob_content);
    let mut decoded_content = String::new();
    decoder.read_to_string(&mut decoded_content).unwrap();

    // Extract the actual "content" from the decompressed data
    let delimiter_index = decoded_content.find('\0').unwrap();

    // Write the content to stdout
    let actual_content = &decoded_content[delimiter_index + 1..];
    print!("{}", actual_content)
}

fn hash_object(path: &str) {
    // Read the content of the file
    let original_content = fs::read(path).unwrap();

    // Create a resulting blob content
    let mut blob_content = Vec::new();
    blob_content.write(b"blob ").unwrap();
    blob_content.write(format!("{}", original_content.len()).as_bytes()).unwrap();
    blob_content.write(b"\0").unwrap();
    blob_content.write(&original_content).unwrap();

    // Calculate sha1 of the blob content
    let object_hash: String = Sha1::digest(&blob_content).iter().map(|x| format!("{:02x}", x)).collect();

    // Create a "partition" directory
    let partition_path = make_object_partition_path(&object_hash);
    fs::create_dir(partition_path).unwrap();

    // Compress the blob using zlib
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&*blob_content).unwrap();
    let compressed_blob_content = encoder.finish().unwrap();

    // Write the blobl content to the file
    let blobl_path = make_object_path(&object_hash);
    let mut blobl_file = fs::File::create(blobl_path).unwrap();
    blobl_file.write(&*compressed_blob_content).unwrap();
    blobl_file.sync_all().unwrap();

    println!("{}", object_hash);
}

fn make_object_partition_path(object_hash: &str) -> path::PathBuf {
    let subdirectory = &object_hash[..2];
    let path: path::PathBuf = [".git", "objects", subdirectory].iter().collect();
    path
}

fn make_object_path(object_hash: &str) -> path::PathBuf {
    let subdirectory = &object_hash[..2];
    let file_name = &object_hash[2..];
    let path: path::PathBuf = [".git", "objects", subdirectory, file_name].iter().collect();
    path
}
