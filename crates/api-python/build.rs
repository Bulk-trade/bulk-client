use std::process::Command;
use std::process::Stdio;

fn main() {
    println!("cargo:rerun-if-changed=always-rerun"); 

    // Runs pip install whenever this "crate" is built
    let status = Command::new("pip")
        .args(&["install", "-e", "."])
        // Directly connect the subprocess output to your terminal
        .stdout(Stdio::inherit()) 
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to execute pip");

    if !status.success() {
        panic!("pip install failed");
    }
}
