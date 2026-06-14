//! `json_to_rust` — read syn-serde JSON, deserialize to `syn::File`, format via
//! `prettyplease::unparse`, write to stdout.
//!
//! Usage: `json_to_rust <input.json>` (or `-` for stdin).
//!
//! Vendored from rules_texlive / rules_lang's `rust_ast_io`, simplified: ratio's
//! Lean-emitted ASTs are shallow, so we use syn-serde's plain `from_str` instead
//! of the serde_stacker recursion-limit dance the deep pdftex corpus needed.

use std::io::{Read, Write};

fn main() {
    if let Err(e) = run() {
        eprintln!("json_to_rust: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = match args.first().map(String::as_str) {
        None | Some("-") => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            s
        }
        Some(path) => std::fs::read_to_string(path)?,
    };

    let file: syn::File = syn_serde::json::from_str(&json)?;
    let formatted = prettyplease::unparse(&file);
    std::io::stdout().lock().write_all(formatted.as_bytes())?;
    Ok(())
}
