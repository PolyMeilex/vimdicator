use std::process::Command;

fn main() {
    if let Ok(output) = Command::new("git").args(["rev-parse", "HEAD"]).output() {
        println!(
            "cargo:rustc-env=GIT_COMMIT={}",
            String::from_utf8(output.stdout).unwrap()
        );
    }
}
