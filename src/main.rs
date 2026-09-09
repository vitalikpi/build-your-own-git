use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use sha1::{Digest, Sha1};
use std::env;
use std::fs;
use std::fs::DirEntry;
use std::io::Read;
use std::io::prelude::*;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
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
        let path: PathBuf = [&args[3]].iter().collect();
        let (_, object_hash) = hash_object(path)?;
        println!("{}", object_hash);
    } else if args[1] == "ls-tree" {
        let sha = &args[3];
        ls_tree(sha)
    } else if args[1] == "write-tree" {
        let path: PathBuf = ["."].iter().collect();
        let (_, sha) = write_tree(path)?;
        println!("{}", sha);
    } else {
        println!("unknown command: {}", args[1])
    }

    Ok(())
}

fn cat_file(object_hash: &str) {
    let path: PathBuf = make_object_path(object_hash);

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

fn write_object(contents: Vec<u8>, object_type: &str) -> anyhow::Result<(Vec<u8>, String)> {
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
    let partition_path = make_object_partition_path(&object_hash);
    fs::create_dir(partition_path)?;

    // Compress the blob using zlib
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&*object_content)?;
    let compressed_blob_content = encoder.finish()?;

    // Write the blob content to the file
    let object_path = make_object_path(&object_hash);
    let mut object_file = fs::File::create(object_path)?;
    object_file.write(&*compressed_blob_content)?;
    object_file.sync_all()?;

    Ok((raw_sha, object_hash))
}

fn hash_object(path: PathBuf) -> anyhow::Result<(Vec<u8>, String)> {
    // Read the content of the file
    let original_content = fs::read(path)?;

    // Write it to the objects database
    write_object(original_content, "blob")
}

fn ls_tree(tree_sha: &String) {
    let path: PathBuf = make_object_path(tree_sha);

    // Read the contents of the blob object file from the .git/objects directory
    let blob_content = fs::read(path).unwrap();

    // Decompress the contents
    let mut decoder = ZlibDecoder::new(&*blob_content);
    let mut decoded_content: Vec<u8> = Vec::new();
    decoder.read_to_end(&mut decoded_content).unwrap();

    let mut i = 0;

    // skip over a first line - the one that contains a object type
    while decoded_content[i] != b'\0' {
        i += 1;
    }
    i += 1;

    while i < decoded_content.len() {
        let mut mode: Vec<u8> = Vec::new();

        while decoded_content[i] != b' ' {
            mode.push(decoded_content[i]);
            i += 1;
        }

        i += 1; // skip the space

        let mut name: Vec<u8> = Vec::new();
        while decoded_content[i] != b'\0' {
            name.push(decoded_content[i]);
            i += 1;
        }

        let mut sha: Vec<u8> = Vec::new();
        i += 1; // skip the space
        let end_of_sha_index = i +20;
        while i < end_of_sha_index {
            sha.push(decoded_content[i]);
            i += 1;
        }

        //let mode_str = String::from_utf8(mode).unwrap();
        let name_str = String::from_utf8(name).unwrap();
        //let sha_str = sha.iter().map(|x| format!("{:02x}", x)).collect();

        //result.push((mode_str, name_str, sha_str));
        println!("{}", name_str)
    }
}

fn write_tree(path: PathBuf) -> anyhow::Result<(Vec<u8>, String)> {
    let mut tree_content: Vec<u8> = Vec::new();

    // read all files
    let mut entries: Vec<DirEntry> = fs::read_dir(path)?
        .map(|x| x.unwrap())
        .collect();

    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        if entry.file_name() == ".git" {
            continue;
        }

        let meta = fs::metadata(&entry.path())?;
        if meta.is_file() {
            if meta.permissions().mode() & 0o111 != 0 { // Executable file
                tree_content.write(b"100755 ")?;
            } else { //  Regular file
                tree_content.write(b"100644 ")?;
            }
            tree_content.write(entry.file_name().into_string().unwrap().as_bytes())?;
            tree_content.write(b"\0")?;
            
            let (sha, _) = hash_object(entry.path())?;
            tree_content.write(&sha)?;
        } else if meta.is_dir() { // 40000
            let (sha, _) = write_tree(entry.path())?;
            tree_content.write(b"40000 ")?;
            tree_content.write(entry.file_name().into_string().unwrap().as_bytes())?;
            tree_content.write(b"\0")?;
            tree_content.write(&sha)?;
        } else {
            eprintln!("Skipped: {}", entry.path().display())
        }
    }

    write_object(tree_content, "tree")
}

fn make_object_partition_path(object_hash: &str) -> PathBuf {
    let subdirectory = &object_hash[..2];
    let path: PathBuf = [".git", "objects", subdirectory].iter().collect();
    path
}

fn make_object_path(object_hash: &str) -> PathBuf {
    let subdirectory = &object_hash[..2];
    let file_name = &object_hash[2..];
    let path: PathBuf = [".git", "objects", subdirectory, file_name].iter().collect();
    path
}
