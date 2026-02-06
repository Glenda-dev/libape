use std::process::Command;

fn main() {
    // Generate timestamp in the format: #1 SMP PREEMPT Fri Feb 6 00:00:00 UTC 2026
    let output = Command::new("date")
        .arg("-u")
        .arg("+#1 UP PREEMPT %a %b %d %H:%M:%S UTC %Y")
        .output()
        .expect("Failed to execute date command");

    let timestamp = String::from_utf8_lossy(&output.stdout);
    let timestamp = timestamp.trim();

    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", timestamp);
    println!("cargo:rerun-if-changed=build.rs");
}
