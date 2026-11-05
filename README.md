# lua51-bytecode

A Rust library that reads compiled Lua 5.1 scripts (the chunks `luac` writes). It also lists the calls a script makes with constant arguments, so scripts that only configure an engine can be read as data without running Lua.

## Features

- Reads Lua 5.1 precompiled chunks in big- or little-endian byte order, with 4- or 8-byte `int` and `size_t`
- Reads the header and every function: source name, line range, upvalue and parameter counts, `is_vararg` and stack size
- Decodes all 38 Lua 5.1 opcodes, and reads the block-number word after a `SETLIST` with `C` 0 as `Op::Data`
- Reads constants (`nil`, booleans, numbers, and strings as bytes) and nested functions
- Reads the debug tables (line numbers, locals and upvalue names) when the chunk is not stripped
- Lists every `CALL` and `TAILCALL` with its callee and arguments, following constants, globals, table fields, method calls and arithmetic on numbers
- Reports for each call the nested function it is in, its instruction index and its source line
- Checks the code with a subset of Lua's own `luaG_checkcode` verifier
- Refuses malformed input with an `Error` that names the byte offset and what was expected there
- Does not panic on malformed input, uses no `unsafe`, and does no file, environment or process I/O
- Sizes no allocation from an unchecked count, and bounds function nesting at 200 levels and call-lister values at 64 nodes
- Tested against 542 chunks that a stock Lua 5.1.5 `luac` compiled from unluac's test sources, plus hand-built hostile chunks

## Getting Started

### Requirements

Rust 1.99 or later (edition 2024). The crate reads Lua 5.1 only; chunks from Lua 5.2 or later, LuaJIT or Luau are refused.

### Installation

```sh
cargo add lua51-bytecode
```

or under `[dependencies]`:

```toml
lua51-bytecode = "0.2"
```

### Usage

`Chunk::parse` reads a whole chunk from a byte slice, and `calls` lists the calls in a function and the functions nested in it. This program parses `print(1.5, "a")` as `luac` writes it on a little-endian machine with 4-byte `int` and `size_t`:

```rust
use lua51_bytecode::{Chunk, Value, calls};
use std::sync::Arc;

fn main() -> Result<(), lua51_bytecode::Error> {
    let bytes: &[u8] = &[
        0x1b, b'L', b'u', b'a', 0x51, 0, 1, 4, 4, 4, 8, 0, // header
        0, 0, 0, 0, // no source name
        0, 0, 0, 0, 0, 0, 0, 0, // line defined, last line defined
        0, 0, 2, 3, // upvalues, parameters, is_vararg, max stack
        5, 0, 0, 0, // five instructions:
        0x05, 0x00, 0x00, 0x00, //   GETGLOBAL R0 K0
        0x41, 0x40, 0x00, 0x00, //   LOADK R1 K1
        0x81, 0x80, 0x00, 0x00, //   LOADK R2 K2
        0x1c, 0x40, 0x80, 0x01, //   CALL R0 B3 C1
        0x1e, 0x00, 0x80, 0x00, //   RETURN R0 B1
        3, 0, 0, 0, // three constants:
        4, 6, 0, 0, 0, b'p', b'r', b'i', b'n', b't', 0, //   "print"
        3, 0, 0, 0, 0, 0, 0, 0xf8, 0x3f, //   1.5
        4, 2, 0, 0, 0, b'a', 0, //   "a"
        0, 0, 0, 0, // no nested functions
        0, 0, 0, 0, // no line numbers
        0, 0, 0, 0, // no locals
        0, 0, 0, 0, // no upvalue names
    ];
    let chunk = Chunk::parse(bytes)?;

    let found = calls(&chunk.main);
    assert_eq!(found[0].callee, Value::Global(Arc::from(&b"print"[..])));
    assert_eq!(found[0].args, [Value::Number(1.5), Value::Str(Arc::from(&b"a"[..]))]);
    Ok(())
}
```

A refused chunk prints as, for example, `Lua 5.1 chunk at byte 4: not Lua 5.1 (version byte is not 0x51)`. The call lister does not follow branches: at every jump target it starts again from nothing known, and a value it cannot know is `Value::Unknown` or, for a call's result, `Value::Result`.

Other examples:

- `cargo run --example list_calls -- script.luac` prints the header of a chunk file and every call it makes.
- `cargo test --doc` runs the crate documentation example, which parses the same chunk.

The full list of what the reader refuses and what the call lister does not do is in [docs/reading-rules.md](docs/reading-rules.md). Byte orders are named with `Endian`, re-exported from [primase-foundations](https://github.com/Kalmarv/primase-foundations).

Run the tests with `cargo test --workspace`; they need no files outside the repository. See [CONTRIBUTING.md](CONTRIBUTING.md) for the full checks and how to report a file the crate cannot read. Licensed under GPL-3.0-or-later; see [LICENSE](LICENSE).
