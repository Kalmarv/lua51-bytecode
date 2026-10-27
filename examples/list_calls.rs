//! Print the header of a compiled Lua 5.1 chunk and the calls its script makes with constant arguments.
//!
//! Run with `cargo run --example list_calls -- script.luac`.
use lua51_bytecode::{Chunk, calls};

fn main() {
    let Some(path) = std::env::args_os().nth(1) else {
        eprintln!("usage: list_calls <compiled-lua-5.1-chunk>");
        std::process::exit(2);
    };
    let name = path.to_string_lossy();
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        eprintln!("{name}: {e}");
        std::process::exit(1);
    });
    let chunk = Chunk::parse(&bytes).unwrap_or_else(|e| {
        eprintln!("{name}: {e}");
        std::process::exit(1);
    });
    println!("{:?}", chunk.header);
    for call in calls(&chunk.main) {
        println!(
            "line {:?} path {:?} pc {}: {:?}({:?}{})",
            call.line,
            call.path,
            call.pc,
            call.callee,
            call.args,
            if call.open_args { ", ..." } else { "" }
        );
    }
}
