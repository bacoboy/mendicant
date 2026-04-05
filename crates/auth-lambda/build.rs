use std::fs;
use std::path::PathBuf;

fn main() {
    let datastar_url = "https://cdn.jsdelivr.net/gh/starfederation/datastar@v1.0.0-RC.8/bundles/datastar.js";
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let static_dir = out_dir.join("static");
    let datastar_path = static_dir.join("datastar.js");

    // Create static directory if it doesn't exist
    fs::create_dir_all(&static_dir).expect("Failed to create static directory");

    // Download Datastar if it doesn't exist
    if !datastar_path.exists() {
        println!("cargo:warning=Downloading Datastar from {}", datastar_url);

        match std::process::Command::new("curl")
            .arg("-fsSL")
            .arg(datastar_url)
            .output()
        {
            Ok(output) if output.status.success() => {
                // Remove source map reference to avoid 404 errors
                let content = String::from_utf8_lossy(&output.stdout);
                let content = content.replace("//# sourceMappingURL=", "//");
                fs::write(&datastar_path, content.as_bytes())
                    .expect("Failed to write datastar.js");
                println!("cargo:warning=Datastar downloaded successfully");
            }
            Ok(output) => {
                eprintln!(
                    "Failed to download Datastar: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                panic!("Failed to download Datastar bundle");
            }
            Err(e) => {
                eprintln!("Failed to run curl: {}", e);
                panic!("Could not download Datastar: {}", e);
            }
        }
    }

    println!("cargo:rerun-if-changed=static/");
}
