use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-env-changed=PHOTO_AI_GIT_COMMIT");

    let commit = std::env::var("PHOTO_AI_GIT_COMMIT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(current_git_commit)
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=PHOTO_AI_GIT_COMMIT={commit}");
}

fn current_git_commit() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let commit = String::from_utf8(output.stdout).ok()?;
    let commit = commit.trim();
    if commit.is_empty() {
        None
    } else {
        Some(commit.to_string())
    }
}
