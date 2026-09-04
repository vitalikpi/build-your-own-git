#[allow(unused_imports)]
use std::env;
#[allow(unused_imports)]
use std::char;
use std::fs;
use std::io::Read;
use std::path;
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use sha1::{Sha1, Digest};
use std::io::prelude::*;

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
        // Make the path to blob in the
        let object_hash = args[3].as_str();
        let subdirectory = &object_hash[..2];
        let file_name = &object_hash[2..];

        let path: path::PathBuf = [".git", "objects", subdirectory, file_name].iter().collect();

        // Read the contents of the blob object file from the .git/objects directory
        let content_bytes = fs::read(path).unwrap();

        // Decompress the contents using Zlib
        let mut decoder = ZlibDecoder::new(&*content_bytes); // TODO: understand WTF is &*

        let mut decoded_content = String::new();
        decoder.read_to_string(&mut decoded_content).unwrap();

        // Extract the actual "content" from the decompressed data
        let delimiter_index = decoded_content.find('\0').unwrap();

        // Write the content to stdout
        let actual_content = &decoded_content[delimiter_index + 1..];
        print!("{}", actual_content)
    } else if args[1] == "hash-object" {
        // $ ./your_program.sh hash-object -w test.txt
        let path = &args[3];

        let content_bytes = fs::read(path).unwrap();

        let object_hash: String = Sha1::digest(&content_bytes).iter().map(|x| format!("{:02x}", x)).collect();


        let subdirectory = &object_hash[..2];
        let file_name = &object_hash[2..];

        let directory_path: path::PathBuf = [".git", "objects", subdirectory].iter().collect();

        fs::create_dir(directory_path).unwrap();

        let file_path: path::PathBuf = [".git", "objects", subdirectory, file_name].iter().collect();

        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(&*content_bytes).unwrap();
        let compressed = e.finish().unwrap();

        let mut file = fs::File::create(file_path).unwrap();
        file.write(b"blob\0").unwrap();
        file.write(&*compressed).unwrap();
        file.sync_all().unwrap();

        println!("{}", object_hash);
    } else {
        println!("unknown command: {}", args[1])
    }
}
