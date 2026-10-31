use crate::object_store::{ObjectType, Sha};
use crate::tree::{Mode, TreeEntry};
use crate::{object_store, tree};
use std::fs;
use std::fs::DirEntry;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

pub fn store_file(path: PathBuf) -> anyhow::Result<Sha> {
    // Read the content of the File
    let original_content = fs::read(path)?;

    // Write it to the objects database
    object_store::write_object(original_content, ObjectType::Blob)
}

pub fn store_dir(path: PathBuf) -> anyhow::Result<Sha> {
    let mut dir_entries: Vec<DirEntry> = fs::read_dir(&path)?
        .map(|x| x.unwrap())
        .collect();
    dir_entries.sort_by_key(|e| e.file_name());

    let mut entries: Vec<TreeEntry> = Vec::new();

    for entry in dir_entries {
        if entry.file_name() == object_store::GIT_DIR { // skip the .git folder
            continue;
        }

        let meta = fs::metadata(&entry.path())?;
        if meta.is_file() {
            let sha = store_file(entry.path())?;

            if meta.permissions().mode() & 0o111 != 0 { // Executable File
                entries.push(TreeEntry {
                    mode: Mode::Executable,
                    name: entry.file_name().into_string().unwrap(),
                    sha
                });
            } else { //  Regular File
                entries.push(TreeEntry {
                    mode: Mode::File,
                    name: entry.file_name().into_string().unwrap(),
                    sha
                });
            }
        } else if meta.is_dir() {
            let name = entry.file_name().into_string().unwrap();
            let sha = store_dir(path.join(&name))?;

            entries.push(TreeEntry {
                mode: Mode::Directory,
                name,
                sha
            });
        } else {
            eprintln!("Skipped: {}", entry.path().display())
        }
    }

    let tree = tree::Tree { entries };
    tree.store()
}

fn checkout_tree(path: &PathBuf, tree_sha: &Sha) -> anyhow::Result<()> {
    let tree = tree::Tree::read(tree_sha)?;

    for entry in tree.entries {
        match entry.mode {
            Mode::File => {
                let (blob_type, _, content) = object_store::read_object(&entry.sha)?;
                assert!(blob_type == ObjectType::Blob);

                let full_path = path.join(entry.name);
                fs::write(full_path, &content)?;
            }
            Mode::Executable => {
                let (blob_type, _, content) = object_store::read_object(&entry.sha)?;
                assert!(blob_type == ObjectType::Blob);

                let full_path = path.join(entry.name);

                fs::write(&full_path, &content)?;

                let mut perms = fs::metadata(&full_path)?.permissions();
                perms.set_mode(100755);
                fs::set_permissions(full_path, perms)?;
            }
            Mode::Directory => {
                let full_path = path.join(entry.name);
                fs::create_dir_all(&full_path)?;
                checkout_tree(&full_path, &entry.sha)?;
            }
            Mode::Symlink => eprintln!("symlinks are not supported"),
            Mode::Submodule => eprintln!("submodules are not supported"),
        }
    }

    Ok(())
}