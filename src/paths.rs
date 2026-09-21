use std::path::PathBuf;
pub fn make_object_path(object_hash: &str) -> (PathBuf, PathBuf) {
    let subdirectory = &object_hash[..2];
    let file_name = &object_hash[2..];
    let partition: PathBuf = [".git", "objects", subdirectory].iter().collect();
    let path: PathBuf = [".git", "objects", subdirectory, file_name].iter().collect();
    (partition, path)
}