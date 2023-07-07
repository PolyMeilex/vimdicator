use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = &env::var("OUT_DIR").unwrap();

    let path = Path::new(out_dir).join("key_map_table.rs");
    let mut file = BufWriter::new(File::create(path).unwrap());

    writeln!(
        &mut file,
        "static KEYVAL_MAP: phf::Map<&'static str, &'static str> = \n{};\n",
        phf_codegen::Map::new()
            .entry("F1", "\"F1\"")
            .entry("F2", "\"F2\"")
            .entry("F3", "\"F3\"")
            .entry("F4", "\"F4\"")
            .entry("F5", "\"F5\"")
            .entry("F6", "\"F6\"")
            .entry("F7", "\"F7\"")
            .entry("F8", "\"F8\"")
            .entry("F9", "\"F9\"")
            .entry("F10", "\"F10\"")
            .entry("F11", "\"F11\"")
            .entry("F12", "\"F12\"")
            .entry("Left", "\"Left\"")
            .entry("Right", "\"Right\"")
            .entry("Up", "\"Up\"")
            .entry("Down", "\"Down\"")
            .entry("Home", "\"Home\"")
            .entry("End", "\"End\"")
            .entry("BackSpace", "\"BS\"")
            .entry("Return", "\"CR\"")
            .entry("Escape", "\"Esc\"")
            .entry("Delete", "\"Del\"")
            .entry("Insert", "\"Insert\"")
            .entry("Page_Up", "\"PageUp\"")
            .entry("Page_Down", "\"PageDown\"")
            .entry("Enter", "\"CR\"")
            .entry("Tab", "\"Tab\"")
            .entry("ISO_Left_Tab", "\"Tab\"")
            .build()
    )
    .unwrap();

    if let Ok(output) = Command::new("git").args(["rev-parse", "HEAD"]).output() {
        println!(
            "cargo:rustc-env=GIT_COMMIT={}",
            String::from_utf8(output.stdout).unwrap()
        );
    }
}
