fn main() {
    // Generate timestamp in the format: #1 SMP PREEMPT Fri Feb 6 00:00:00 UTC 2026
    let now = chrono::Utc::now();
    let timestamp = now.format("#1 SMP PREEMPT %a %b %d %H:%M:%S UTC %Y").to_string();
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    let arch = target.split('-').next().unwrap_or("unknown");
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", timestamp);
    println!("cargo:rustc-env=ARCH={}", arch);
    println!("cargo:rerun-if-changed=build.rs");
}
