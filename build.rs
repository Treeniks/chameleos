use std::path::Path;
use std::process::Command;
use std::process::ExitStatus;

fn run_command<const N: usize>(command: [&str; N]) -> (ExitStatus, String) {
    let s = Command::new(command[0])
        .args(&command[1..])
        .output()
        .unwrap();
    (
        s.status,
        String::from_utf8(s.stdout).unwrap().trim().to_owned(),
    )
}

fn main() {
    println!("cargo::rerun-if-changed=.git");
    println!("cargo::rerun-if-changed=src");

    let version = {
        let (status, describe) = run_command(["git", "describe", "--exact-match", "--tags"]);
        status
            .success()
            .then_some(describe)
            .or_else(|| {
                let (status, describe) = run_command(["git", "describe", "--long", "--tags"]);
                status.success().then_some(describe)
            })
            .unwrap_or_else(|| format!("v{}", std::env::var("CARGO_PKG_VERSION").unwrap()))
    };

    let commit_hash = run_command(["git", "rev-parse", "HEAD"]).1;

    let rustc_version = run_command([&std::env::var("RUSTC").unwrap(), "--version"]).1;

    let build_time = format!("{}", chrono::Local::now());

    let target = {
        let mut s = format!(
            "{}-{}-{}",
            std::env::var("CARGO_CFG_TARGET_ARCH").unwrap(),
            std::env::var("CARGO_CFG_TARGET_VENDOR").unwrap(),
            std::env::var("CARGO_CFG_TARGET_OS").unwrap(),
        );

        let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap();
        if !env.is_empty() {
            s.push('-');
            s.push_str(&env);
        }
        s
    };

    let mut long_version = format!("{version}",);
    if !commit_hash.is_empty() {
        long_version.push_str(&format!("\ncommit hash: {commit_hash}"));
    }
    long_version.push_str(&format!(
        "\nbuild time: {build_time}\n{rustc_version}\n{target}"
    ));

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("metadata.rs");
    std::fs::write(
        dest_path,
        format!(
            r#"
pub const VERSION: &str = "{version}";
pub const LONG_VERSION: &str = "{long_version}";
"#
        ),
    )
    .unwrap();
}
