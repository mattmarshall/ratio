//! `rust_to_json` — parse Rust source (file arg or stdin) to `syn::File`,
//! serialize to pretty syn-serde JSON on stdout. The companion to
//! `json_to_rust`; used to discover the exact syn-serde shape a new
//! `Polyglot.Rust` builder must produce (the "inspect-cycle").

use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let src = match args.first().map(String::as_str) {
        None | Some("-") => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).unwrap();
            s
        }
        Some(p) => std::fs::read_to_string(p).unwrap(),
    };
    let file: syn::File = syn::parse_file(&src).expect("parse Rust source");
    println!("{}", syn_serde::json::to_string_pretty(&file));
}
