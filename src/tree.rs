use crate::object_store::{ObjectType, Sha};
use crate::object_store;
use std::io::Write;

#[derive(PartialEq)]
pub enum Mode {
    File,
    Executable,
    Directory,
    Symlink,
    Submodule,
}

#[derive(PartialEq)]
pub struct TreeEntry {
    pub mode: Mode,
    pub name: String,
    pub sha: Sha,
}

pub struct Tree {
    pub entries: Vec<TreeEntry>,
}

impl Tree {
    pub fn read(sha: &Sha) -> anyhow::Result<Self> {
        let (object_type, _, decoded_content) = object_store::read_object(sha)?;

        assert!(object_type == ObjectType::Tree);

        Self::parse(decoded_content)
    }

    pub fn parse(content: Vec<u8>) -> anyhow::Result<Self> {
        let mut entries: Vec<TreeEntry> = Vec::new();

        let mut i = 0;
        while i < content.len() {
            let mut mode: Vec<u8> = Vec::new();

            while content[i] != b' ' {
                mode.push(content[i]);
                i += 1;
            }

            i += 1; // skip the space

            let mut name: Vec<u8> = Vec::new();
            while content[i] != b'\0' {
                name.push(content[i]);
                i += 1;
            }

            let mut sha: Vec<u8> = Vec::new();
            i += 1; // skip the space
            let end_of_sha_index = i + 20;
            while i < end_of_sha_index {
                sha.push(content[i]);
                i += 1;
            }

            let mode_str = String::from_utf8(mode)?;
            let name_str = String::from_utf8(name)?;

            let mode: Mode = match mode_str.as_str() {
                "100644" => Mode::File,
                "100755" => Mode::Executable,
                "40000" => Mode::Directory,
                "120000" => Mode::Symlink,
                "160000" => Mode::Submodule,
                _ => panic!("unknown object type")
            };

            entries.push(TreeEntry { mode, name: name_str, sha: Sha::new(&sha) });
        }

        Ok(Self { entries })
    }

    pub fn store(&self) -> anyhow::Result<Sha> {
        let mut tree_content: Vec<u8> = Vec::new();

        let mut entries: Vec<&TreeEntry> = self.entries.iter().collect();

        entries.sort_by_key(|e| &e.name);

        for entry in entries {
            match entry.mode {
                Mode::File => {
                    tree_content.write(b"100644 ")?;
                    tree_content.write(entry.name.as_bytes())?;
                    tree_content.write(b"\0")?;
                }
                Mode::Executable => {
                    tree_content.write(b"100755 ")?;
                    tree_content.write(entry.name.as_bytes())?;
                    tree_content.write(b"\0")?;
                }
                Mode::Directory => {
                    tree_content.write(b"40000 ")?;
                    tree_content.write(entry.name.as_bytes())?;
                    tree_content.write(b"\0")?;
                    tree_content.write(&entry.sha.value)?; // TODO: is this correct
                }
                Mode::Symlink => eprintln!("Skipped: {}", entry.name),
                Mode::Submodule => eprintln!("Skipped: {}", entry.name),
            }
        }

        object_store::write_object(tree_content, ObjectType::Tree)
    }
}