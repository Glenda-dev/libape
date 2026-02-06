fn main() {
    // Generate timestamp in the format: #1 SMP PREEMPT Fri Feb 6 00:00:00 UTC 2026
    let now = chrono::Utc::now();
    let timestamp = now.format("#1 SMP PREEMPT %a %b %d %H:%M:%S UTC %Y").to_string();

    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", timestamp);
    println!("cargo:rerun-if-changed=build.rs");
}
