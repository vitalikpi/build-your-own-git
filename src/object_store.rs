use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use sha1::{Digest, Sha1};
use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use anyhow::Context;

pub static GIT_DIR: &str = ".git";

pub fn make_object_path(object_hash: &Sha) -> (PathBuf, PathBuf) {
    let sha_str = object_hash.to_string();
    let subdirectory = &sha_str[..2];
    let file_name = &sha_str[2..];
    let partition: PathBuf = [GIT_DIR, "objects", subdirectory].iter().collect();
    let path: PathBuf = [GIT_DIR, "objects", subdirectory, file_name].iter().collect();
    (partition, path)
}

#[derive(PartialEq)]
pub enum ObjectType {
    Blob,
    Commit,
    Tree,
    Tag
}

#[derive(PartialEq)]
pub struct Sha {
    pub value: [u8; 20]
}

impl Sha {
    pub fn new(object_content: &Vec<u8>) -> Self {
        let value: [u8; 20] = Sha1::digest(&object_content).into();
        Self { value }
    }

    pub fn from_string(s: &str) -> anyhow::Result<Self> {
        if s.len() != 40 {
            anyhow::bail!("sha string should be exactly 40 characters long");
        }
        let mut out = [0u8; 20];
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(s.get(i * 2..i * 2 + 2).context("invalid sha")?, 16)?;
        }
        Ok(Self {
            value: out
        })
    }
    pub fn to_string(&self) -> String {
        self.value.clone().into_iter().map(|x| format!("{:02x}", x)).collect()
    }
}

pub fn init() -> anyhow::Result<()> {
    fs::create_dir(GIT_DIR)?;
    fs::create_dir([GIT_DIR, "objects"].iter().collect::<PathBuf>())?;
    fs::create_dir([GIT_DIR, "refs"].iter().collect::<PathBuf>())?;
    fs::write([GIT_DIR, "HEAD"].iter().collect::<PathBuf>(), "ref: refs/heads/main\n")?;
    Ok(())
}

pub fn read_object(object_hash: &Sha) -> anyhow::Result<(ObjectType, usize, Vec<u8>)> {
    let (_, path) = make_object_path(&object_hash);

    // Read the contents of the blob object File from the .git/objects Directory
    let blob_content = fs::read(path)?;

    // Decompress the contents
    let mut decoder = ZlibDecoder::new(&*blob_content);
    let mut decoded_content: Vec<u8> = Vec::new();
    decoder.read_to_end(&mut decoded_content)?;

    // Extract the actual "content" from the decompressed data
    let mut delimiter_index = 0; // = decoded_content.find('\0').unwrap();
    let mut object_type: String = String::new();
    while decoded_content[delimiter_index] != b' ' {
        object_type.push(char::from(decoded_content[delimiter_index]));
        delimiter_index += 1;
    }

    let object_type = match object_type.as_str() {
        "blob" => ObjectType::Blob,
        "tree" => ObjectType::Tree,
        "commit" => ObjectType::Commit,
        "tag" => ObjectType::Tag,
        _ => panic!("unexpected object type")
    };

    delimiter_index += 1;

    let mut size: String = String::new();
    while decoded_content[delimiter_index] != b'\0' {
        size.push( char::from(decoded_content[delimiter_index]));
        delimiter_index += 1;
    }
    let size: usize = size.parse()?;

    decoded_content.drain(0..delimiter_index);

    Ok((object_type, size, decoded_content))
}

pub fn write_object(contents: Vec<u8>, object_type: ObjectType) -> anyhow::Result<Sha> {
    let object_type: &str = match object_type {
        ObjectType::Blob => "blob",
        ObjectType::Commit => "commit",
        ObjectType::Tree => "tree",
        ObjectType::Tag => "tag"
    };

    let mut object_content = Vec::new();
    object_content.write(object_type.as_bytes())?;
    object_content.write(b" ")?;
    object_content.write(format!("{}", contents.len()).as_bytes())?;
    object_content.write(b"\0")?;
    object_content.write(&contents)?;

    // Calculate sha1 of the blob content
    let sha = Sha::new(&object_content);

    // Create a "partition" Directory
    let (partition_path, object_path) = make_object_path(&sha);
    fs::create_dir_all(partition_path)?;

    // Compress the blob using zlib
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&*object_content)?;
    let compressed_blob_content = encoder.finish()?;

    // Write the blob content to the File
    let mut object_file = fs::File::create(object_path)?;
    object_file.write(&*compressed_blob_content)?;
    object_file.sync_all()?;

    Ok(sha)
}