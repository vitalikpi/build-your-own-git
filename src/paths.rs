use std::path::PathBuf;

pub static GIT_DIR: &str = ".mygit";

pub fn make_object_path(object_hash: &str) -> (PathBuf, PathBuf) {
    let subdirectory = &object_hash[..2];
    let file_name = &object_hash[2..];
    let partition: PathBuf = [GIT_DIR, "objects", subdirectory].iter().collect();
    let path: PathBuf = [GIT_DIR, "objects", subdirectory, file_name].iter().collect();
    (partition, path)
}