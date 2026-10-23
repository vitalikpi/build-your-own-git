use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::{env, io};
use std::fs;
use std::fs::DirEntry;
use std::io::Read;
use std::io::prelude::*;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

mod paths;

fn parse_args() -> anyhow::Result<(String, HashMap<String, String>, Vec<String>)>{
    let command: String;
    let mut named_arguments: HashMap<String, String> = HashMap::new();
    let mut positional_arguments: Vec<String> = Vec::new();

    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 { anyhow::bail!("No command provided.") }
    if args[1].starts_with("-") { anyhow::bail!("Command can't start with -") }

    command = args[1].clone();

    let mut current_parameter_name: Option<String> = None;
    for arg in args.iter().skip(2) {
        if arg.starts_with("-") {
            match current_parameter_name {
                None => {
                    current_parameter_name = Some(arg.clone());
                }
                Some(parameter_name) => {
                    named_arguments.insert(parameter_name, "".to_string());
                    current_parameter_name = Some(arg.clone())
                }
            }
        } else {
            match current_parameter_name {
                None => {
                    positional_arguments.push(arg.clone())
                }
                Some(parameter_name) => {
                    named_arguments.insert(parameter_name, arg.to_string());
                    current_parameter_name = None
                }
            }
        }
    }

    Ok((command, named_arguments, positional_arguments))
}

fn main() -> anyhow::Result<()> {
    let (command, named_arguments, positional_arguments) = parse_args()?;

    if command == "init" {
        fs::create_dir(".git")?;
        fs::create_dir(".git/objects")?;
        fs::create_dir(".git/refs")?;
        fs::write(".git/HEAD", "ref: refs/heads/main\n")?;
        println!("Initialized git directory");
    } else if command == "cat-file" {
        match named_arguments.get("-p") {
            None => eprintln!("No sha provided"),
            Some(object_hash) => {
                let content = read_object(object_hash)?;
                io::stdout().write_all(&content)?;
            }
        }
    } else if command == "hash-object" {
        match named_arguments.get("-w") {
            None => {
                eprintln!("No path provided")
            }
            Some(path) => {
                let path: PathBuf = [path].iter().collect();
                let (_, object_hash) = hash_object(path)?;
                println!("{}", object_hash);
            }
        }
    } else if command ==  "ls-tree" {
        match named_arguments.get("--name-only") {
            None => { eprintln!("No sha provided") }
            Some(sha) => { ls_tree(sha) }
        }
    } else if command ==  "write-tree" {
        let path: PathBuf = ["."].iter().collect();
        let (_, sha) = write_tree(path)?;
        println!("{}", sha);
    } else if command == "commit-tree" {
        let tree_sha = positional_arguments.first().unwrap();
        let parent_commit_sha = named_arguments.get("-p");
        let commit_message = named_arguments.get("-m");

        let (_, commit_sha) = commit_tree(tree_sha, parent_commit_sha, commit_message)?;
        println!("{}", commit_sha);
//    } else if command == "checkout-empty" {
//        let commit_sha = positional_arguments.first().unwrap();

//        let (_, path) = make_object_path(commit_sha);
//        let updated_paths = checkout_empty(commit_sha);

//        println!("Updated {} path(s) from the index", 0)
    } else if command ==  "help" {
        println!("Hello, World!")
    } else {
        println!("unknown command: {}", command)
    }

    Ok(())
}

/*
fn checkout_empty(commit_sha: &String) -> anyhow::Result<u8> {
    let (_, object_path) = make_object_path(commit_sha);



    return Ok(0);
}

fn checkout_tree(tree_sha: &String) -> anyhow::Result<u8> {
    return Ok(0);
}
*/

fn read_object(object_hash: &str) -> anyhow::Result<Vec<u8>> {
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
    let (partition_path, object_path) = paths::make_object_path(&object_hash);
    fs::create_dir(partition_path)?;

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

//fn read_object(sha)

fn hash_object(path: PathBuf) -> anyhow::Result<(Vec<u8>, String)> {
    // Read the content of the file
    let original_content = fs::read(path)?;

    // Write it to the objects database
    write_object(original_content, "blob")
}

fn ls_tree(tree_sha: &String) {
    let (_, path) = paths::make_object_path(tree_sha);

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

fn commit_tree(tree_sha: &String, parent_commit_sha: Option<&String>, message: Option<&String>) -> anyhow::Result<(Vec<u8>, String)> {
    // tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904
    // parent 3b18e512dba79e4c8300dd08aeb37f8e728b8dad
    // author John Doe <john@example.com> 1234567890 +0000
    // committer John Doe <john@example.com> 1234567890 +0000
    //
    // Initial commit
    let mut commit_content: String = "tree ".to_string();

    commit_content.push_str(tree_sha);
    commit_content.push_str("\n");

    match parent_commit_sha {
        Some(parent_commit_sha) => {
            commit_content.push_str("parent ");
            commit_content.push_str(parent_commit_sha);
            commit_content.push_str("\n");
        }
        None => {}
    }

    commit_content.push_str("author John Doe <john@example.com> 1234567890 +0000");
    commit_content.push_str("\n");
    commit_content.push_str("\n");

    match message {
        Some(message) => {
            commit_content.push_str(message);
            commit_content.push_str("\n");
        }
        None => {}
    }

    write_object(commit_content.into_bytes(), "commit")
}

