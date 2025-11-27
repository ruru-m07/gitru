fn main() {
    println!("cargo:warning=Running post-build typegen script...");
    let output = std::process::Command::new("sh")
        .arg("scripts/typegen.sh")
        .current_dir("../../../")
        .output()
        .expect("Failed to run typegen.sh script");

    if !output.status.success() {
        eprintln!("Script failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("typegen.sh script failed");
    }
    println!("cargo:warning=Typegen script completed successfully");

    tauri_build::build()
}
