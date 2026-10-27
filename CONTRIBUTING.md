# Contributing

## Build and test

The toolchain is pinned in `rust-toolchain.toml`.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

The tests use chunks written out byte by byte in `tests/chunks.rs`; they need no external files.

## No I/O

The library parses byte slices. It does not read files, the environment or processes: `clippy.toml` lists those functions as disallowed and each source file turns that into an error. Examples and tests may use them.

## Reporting a file it can't read

Open an issue with the "Unreadable file" template. Please include:

- The error text, which names the byte offset and what was expected there.
- The first 64 bytes of the file as hex (for example `xxd -l 64 file`) and its total size.
- Where the script comes from (the game or program, and its version) and how it was compiled, if known (`luac -v`, or the game's own compiler).
- If the error is at a later offset, the 32 bytes around it as hex.

Please do not attach game files, scripts you do not have the right to share, or anything that contains personal data. If a small chunk that reproduces the problem can be made (for example by compiling a tiny script yourself with `luac5.1`), attach that instead.

## Pull requests

Keep changes small and tested. A new refusal needs a test that writes the offending bytes and checks the offset. Public items need doc comments; `cargo doc` must pass with warnings denied.
