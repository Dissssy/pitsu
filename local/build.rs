extern crate embed_resource;

fn main() {
    #[cfg(target_os = "windows")]
    match embed_resource::compile("manifest.rc", embed_resource::NONE) {
        embed_resource::CompilationResult::NotWindows => {
            println!("Not compiling resource file on non-Windows platform.");
        }
        embed_resource::CompilationResult::Ok => {
            println!("Resource file compiled successfully.");
        }
        _ => {
            eprintln!("Failed to compile resource file.");
            std::process::exit(1);
        }
    };
}
