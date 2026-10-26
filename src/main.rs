use std::collections::HashMap;
use std::fs;
use std::fs::DirEntry;
use std::io::prelude::*;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::{env, io};
use thiserror::__private::AsDisplay;

mod paths;
mod object_store;

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
        fs::create_dir(paths::GIT_DIR)?;
        fs::create_dir([paths::GIT_DIR, "objects"].iter().collect::<PathBuf>())?;
        fs::create_dir([paths::GIT_DIR, "refs"].iter().collect::<PathBuf>())?;
        fs::write([paths::GIT_DIR, "HEAD"].iter().collect::<PathBuf>(), "ref: refs/heads/main\n")?;
        println!("Initialized git directory");
    } else if command == "cat-file" {
        match named_arguments.get("-p") {
            None => eprintln!("No sha provided"),
            Some(object_hash) => {
                let content = object_store::read_object(object_hash)?;
                io::stdout().write_all(&content)?;
            }
        }
    } else if command == "hash-object" {
        match named_arguments.get("-w") {
            None => { eprintln!("No path provided")}
            Some(path) => {
                let path: PathBuf = [path].iter().collect();
                let (_, object_hash) = hash_object(path)?;
                println!("{}", object_hash);
            }
        }
    } else if command ==  "ls-tree" {
        match named_arguments.get("--name-only") {
            None => eprintln!("No sha provided"),
            Some(sha) => ls_tree(sha)?,
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
    } else if command == "checkout-empty" {
        let commit_sha = positional_arguments.first().unwrap();

        let (_, path) = paths::make_object_path(commit_sha);
        let updated_paths = checkout_empty(commit_sha);

        println!("Updated {} path(s) from the index", 0)
    } else if command ==  "help" {
        println!("Hello, World!")
    } else {
        println!("unknown command: {}", command)
    }

    Ok(())
}

fn hash_object(path: PathBuf) -> anyhow::Result<(Vec<u8>, String)> {
    // Read the content of the file
    let original_content = fs::read(path)?;

    // Write it to the objects database
    object_store::write_object(original_content, "blob")
}

fn ls_tree(tree_sha: &String) -> anyhow::Result<()>{
    let decoded_content = object_store::read_object(tree_sha)?;

    let mut i = 0;
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

    Ok(())
}

fn write_tree(path: PathBuf) -> anyhow::Result<(Vec<u8>, String)> {
    println!("{}", path.as_display());
    let mut tree_content: Vec<u8> = Vec::new();

    // read all files
    let mut entries: Vec<DirEntry> = fs::read_dir(path)?
        .map(|x| x.unwrap())
        .collect();

    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        if entry.file_name() == paths::GIT_DIR || entry.file_name().to_string_lossy().starts_with('.') {
            // skip the .git folder and anything that starts with . - this is for educational purpose only - this is fine
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

    object_store::write_object(tree_content, "tree")
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

    object_store::write_object(commit_content.into_bytes(), "commit")
}

fn checkout_empty(commit_sha: &String) -> anyhow::Result<u8> {
    let content = object_store::read_object(commit_sha);

    return Ok(0);
}
