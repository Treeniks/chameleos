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

fn rerun_if_changed(manifest_dir: &str, git: bool) {
    let mut rerun_if_changed = vec![Path::new("build.rs").to_path_buf()];

    if git {
        let (status, git_dir) = run_command(["git", "-C", manifest_dir, "rev-parse", "--git-dir"]);
        if let Some(git_dir) = status.success().then_some(git_dir) {
            let git_dir = Path::new(&manifest_dir).join(git_dir);
            rerun_if_changed.push(git_dir.join("HEAD"));
        }

        let (status, git_common_dir) =
            run_command(["git", "-C", manifest_dir, "rev-parse", "--git-common-dir"]);
        if let Some(git_common_dir) = status.success().then_some(git_common_dir) {
            let git_common_dir = Path::new(&manifest_dir).join(git_common_dir);
            rerun_if_changed.push(git_common_dir.join("refs").join("heads"));
            rerun_if_changed.push(git_common_dir.join("refs").join("tags"));
            rerun_if_changed.push(git_common_dir.join("packed-refs"));
        }
    }

    for path in rerun_if_changed {
        println!("cargo::rerun-if-changed={}", path.display());
    }
}

fn main() {
    // only run git commands if we're in an actual checkout of this repo
    // otherwise git commands may report information about a parent dir's repo
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let git = Path::new(&manifest_dir).join(".git").exists();
    rerun_if_changed(&manifest_dir, git);

    let version = git
        .then(|| run_command(["git", "-C", &manifest_dir, "describe", "--tags"]))
        .and_then(|(status, describe)| status.success().then_some(describe))
        .unwrap_or_else(|| format!("v{}", std::env::var("CARGO_PKG_VERSION").unwrap()));

    let commit_hash = git
        .then(|| run_command(["git", "-C", &manifest_dir, "rev-parse", "HEAD"]))
        .and_then(|(status, hash)| status.success().then_some(hash));

    let build_time = format!("{}", chrono::Local::now());

    let rustc_version = run_command([&std::env::var("RUSTC").unwrap(), "--version"]).1;

    let target = std::env::var("TARGET").unwrap();

    let mut long_version = version.clone();
    if let Some(hash) = commit_hash {
        long_version.push_str(&format!("\ncommit hash: {hash}"));
    }
    long_version.push_str(&format!(
        "\nbuild time: {build_time}\n{rustc_version}\n{target}"
    ));

    println!("cargo::rustc-env=CHAMELEOS_VERSION={version}");
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let long_version_path = Path::new(&out_dir).join("long_version");
    std::fs::write(&long_version_path, long_version).unwrap();
}
