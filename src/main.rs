#[allow(unused_imports)]
use std::env;
#[allow(unused_imports)]
use std::char;
use std::fs;
use std::io::Read;
use std::path;
use flate2::read::ZlibDecoder;

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
        let subdirectory = &object_hash[..2]; // WTF do i do here?
        let file_name = &object_hash[2..];

        let path: path::PathBuf = [".git", "objects", subdirectory, file_name].iter().collect();

        //eprintln!("{}", &path.display());

        // Read the contents of the blob object file from the .git/objects directory
        let content_bytes = fs::read(path).unwrap();

        // Decompress the contents using Zlib
        let mut decoder = ZlibDecoder::new(&*content_bytes); // TODO: understand WTF is &*

        let mut decoded_content = String::new();
        decoder.read_to_string(&mut decoded_content).unwrap();

        // Extract the actual "content" from the decompressed data
        let delimiter_index = decoded_content.find('\0').unwrap();

        // Write the content to stdout
        let actual_content = &decoded_content[delimiter_index+1..];
        println!("{}", actual_content)


    } else {
        println!("unknown command: {}", args[1])
    }
}
