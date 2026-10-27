# lua51-bytecode

A strict Rust reader for compiled Lua 5.1 scripts (`luac` output) that checks untrusted bytes and refuses what `luac` would never write, plus a lister of the calls a script makes with constant arguments.

## What it reads

- **Lua 5.1 precompiled chunks** (`ESC Lua`, version `0x51`, format 0) in either byte order, with 4- or 8-byte `int` and `size_t`, 4-byte instructions and 8-byte floating-point numbers.
- Everything in a chunk: the header, each function prototype (source name, line range, upvalue and parameter counts, `is_vararg`, stack size), the decoded instructions (all 38 Lua 5.1 opcodes), the constants (`nil`, booleans, numbers, strings as bytes), nested functions, and the debug tables (line numbers, locals, upvalue names) when the chunk is not stripped.
- **Calls with constant arguments.** `calls(&chunk.main)` follows registers through `LOADK`, `LOADBOOL`, `LOADNIL`, `MOVE`, `GETGLOBAL`, `GETTABLE`, `SELF` and arithmetic on numbers, and reports every `CALL` and `TAILCALL` with what its callee and arguments held. This is how a script that only configures an engine, such as `change_fog(400, 700, -10)` or `level.LoadData(level, "path")`, reads without running a Lua VM.

It reads Lua 5.1 only (not 5.2 or later, LuaJIT or Luau chunks). It is checked against chunks written byte by byte from the Lua 5.1 sources (`lopcodes.h`, `lundump.c`) and hostile variants of them; this repository holds no scripts from any game.

## Example

```rust
use lua51_bytecode::{Chunk, Value, calls};
use std::sync::Arc;

fn main() -> Result<(), lua51_bytecode::Error> {
    // `print(1.5, "a")` as `luac` writes it for a little-endian machine with 4-byte `int` and `size_t`.
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

The same example runs as a doc test of the crate (`cargo test --doc`). To try the reader on a file, `cargo run --example list_calls -- script.luac` prints the header and the calls.

Add it with `cargo add lua51-bytecode` (or `lua51-bytecode = "0.1"` under `[dependencies]`). Its API names byte orders with `Endian`, which it re-exports from the [`primase-foundations`](https://github.com/Kalmarv/primase-foundations) crate, a small byte-cursor library it depends on.

## What it guarantees

- **Checked reads.** `Chunk::parse` takes a byte slice, uses no `unsafe` (`unsafe_code` is denied), and the library source denies unchecked indexing, unchecked arithmetic and truncating casts at compile time. Malformed input returns an `Error`; it does not panic.
- **Refusals name the offset and what was expected.** `Error { offset, detail }` prints as, for instance, `Lua 5.1 chunk at byte 4: not Lua 5.1 (version byte is not 0x51)`. Refused: any other header; a count the rest of the chunk cannot hold at the least size of its items (no allocation is sized by an unchecked count); a string without its terminating NUL; a boolean other than 0 or 1; an unknown constant type; `is_vararg` bits Lua 5.1 does not define; a stack past 250 registers; line numbers for some but not all instructions; a local whose range leaves the code; upvalue names for some but not all upvalues; more than 200 functions nested inside one another (`MAX_DEPTH`); bytes after the main function; and code the verifier rejects.
- **A verifier for the code.** A subset of Lua's own `luaG_checkcode` that needs no symbolic execution: an opcode that does not exist, a register past the stack, a constant, function or upvalue past its table, a `GETGLOBAL` or `SETGLOBAL` whose constant is not a string, a jump out of the code, a test not followed by a jump, a `LOADBOOL` that skips past the end, a `CLOSURE` not followed by its upvalue instructions, and a function that does not end in `RETURN`.
- **The call lister does not guess.** It does not run branches or loops: every instruction a jump can reach starts from nothing known, so a value that came through control flow, an upvalue or a call's result is `Value::Unknown` (or `Value::Result`, naming the call), never a guess.

## What it does not do

- It does not execute Lua, and it does not verify chunks as fully as the Lua VM's loader (no symbolic execution of registers).
- It does not write chunks, print disassembly, or convert between byte orders or sizes.
- The call lister lists a call under a condition as if it were unconditional (its arguments are those known on the straight path to it), and does not follow values through upvalues, tables built at run time, or arithmetic that is not on numbers.
- Chunks with `int` or `size_t` sizes other than 4 or 8, instructions other than 4 bytes, or integral (non-float) numbers are refused, not read.

## Tests

```sh
cargo test --workspace
```

`tests/hostile.rs` and `tests/allocation.rs` hold hand-built hostile chunks: wrong offsets, counts larger than the bytes present (with a counting allocator), and field chains meant to blow up the call lister. `tests/chunks.rs` reads chunks written out byte by byte from `lopcodes.h` and `lundump.c` (the same function big- and little-endian; one that folds `-10`, jumps and nests a method call; constants of every type). It refuses each header field, every truncation of each of them and a trailing byte, and each rule of the verifier at the byte that breaks it. The tests need no game files or other data, so there is no `GAME_DATA` file to set up.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md), including how to report a file this crate cannot read.

## License

GPL-3.0-or-later; see [LICENSE](LICENSE).

Used by [primase](https://github.com/Kalmarv/primase), a game-asset conversion library.
