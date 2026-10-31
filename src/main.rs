use crate::filesystem::{store_dir, store_file};
use crate::object_store::Sha;
use crate::tree::Tree;
use std::collections::HashMap;
use std::io::prelude::*;
use std::path::PathBuf;
use std::{env, io};
use anyhow::Context;

mod object_store;
mod tree;
mod commit;
mod filesystem;

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
        object_store::init()?;
        println!("Initialized git Directory");
    } else if command == "cat-file" {
        let object_hash = named_arguments.get("-p").context("No sha provided")?;
        let sha = Sha::from_string(object_hash).context("Invalid sha")?;
        let (_, _, content) = object_store::read_object(&sha)?;
        io::stdout().write_all(&content)?;
    } else if command == "hash-object" {
        let path = named_arguments.get("-w").context("No path provided")?;
        let path: PathBuf = [path].iter().collect();
        let sha = store_file(path)?;
        println!("{}", sha.to_string());
    } else if command ==  "ls-tree" {
        let sha_str = named_arguments.get("--name-only").context("No sha provided")?;
        let sha = Sha::from_string(sha_str).context("invalid tree sha")?;
        for entry in Tree::read(&sha)?.entries {
            println!("{}", entry.name)
        }
    } else if command ==  "write-tree" {
        let sha = store_dir(["."].iter().collect())?;
        println!("{}", sha.to_string());
    } else if command == "commit-tree" {
        let tree_sha_str = positional_arguments.first().context("missing tree sha")?;
        let tree_sha = Sha::from_string(tree_sha_str).context("invalid tree sha")?;

        let parent_sha = named_arguments.get("-p").map(|s| Sha::from_string(s)).transpose()?;

        let message = named_arguments.get("-m").cloned();

        let commit = commit::Commit {
            tree_sha,
            parent_sha,
            message,
            commiter: None
        };

        let commit_sha = commit.store()?;
        println!("{}", commit_sha.to_string());
    } else if command == "checkout-empty" {
        let commit_sha = positional_arguments.first().unwrap();

        _ = checkout_empty(commit_sha);
        println!("Updated {} path(s) from the index", 0)
    } else if command ==  "help" {
        println!("Hello, World!")
    } else {
        println!("unknown command: {}", command)
    }

    Ok(())
}