# Reading rules

What `Chunk::parse` accepts and refuses, how the code verifier works, and the limits of `calls`.

## What is read

- Lua 5.1 precompiled chunks (`ESC Lua`, version `0x51`, format 0) in either byte order, with 4- or 8-byte `int` and `size_t`, 4-byte instructions and 8-byte floating-point numbers.
- The header, each function prototype (source name, line range, upvalue and parameter counts, `is_vararg`, stack size), the decoded instructions (all 38 Lua 5.1 opcodes, plus `Op::Data` for the block-number word after `SETLIST` with `C` 0), the constants, nested functions, and the debug tables when the chunk is not stripped.

## Checked reads

`Chunk::parse` takes a byte slice and uses no `unsafe` (`unsafe_code` is denied). The library source denies unchecked indexing, unchecked arithmetic and truncating casts at compile time, and `clippy.toml` with a `deny` in each source file keeps file, environment and process access out of it. Malformed input returns an `Error { offset, detail }`; it does not panic.

## Refusals

A chunk is refused, at the offset of the field at fault, for:

- any header other than the one above;
- a count the rest of the chunk cannot hold at the least size of its items (no allocation is sized by an unchecked count);
- a string without its terminating NUL;
- a boolean constant other than 0 or 1, or an unknown constant type;
- `is_vararg` bits Lua 5.1 does not define;
- a stack of more than 250 registers;
- line numbers for some but not all instructions;
- a local whose range is reversed or leaves the code;
- upvalue names for some but not all upvalues;
- more than 200 functions nested inside one another (`MAX_DEPTH`);
- bytes after the main function;
- code the verifier rejects.

## The verifier

A subset of Lua's own `luaG_checkcode` that needs no symbolic execution. It refuses:

- an opcode that does not exist;
- a register past the stack;
- a constant, function or upvalue past its table;
- a `GETGLOBAL` or `SETGLOBAL` whose constant is not a string;
- a jump out of the code;
- a `SETLIST` with `C` 0 that has no block number after it, or a data word anywhere else;
- a test not followed by a jump;
- a `LOADBOOL` that skips past the end;
- a `CLOSURE` not followed by its upvalue instructions;
- a function that does not end in `RETURN`.

## The call lister

`calls` follows registers through `LOADK`, `LOADBOOL`, `LOADNIL`, `MOVE`, `GETGLOBAL`, `GETTABLE`, `SELF`, `UNM` and arithmetic on numbers (folded in `f64` as the VM would), and reports every `CALL` and `TAILCALL`.

- It does not run branches or loops: every instruction a jump can reach starts from nothing known.
- A value that came through control flow, an upvalue, or any other operation is `Value::Unknown`; a call's result is `Value::Result(pc)`.
- A call under a condition is listed as if it were unconditional, with its arguments as known on the straight path to it.
- A field chain of more than `MAX_VALUE_NODES` (64) nodes becomes `Value::Unknown`.

## What it does not do

- It does not execute Lua, and it does not verify chunks as fully as the Lua VM's loader.
- It does not write chunks, print disassembly, or convert between byte orders or sizes.
- Chunks with `int` or `size_t` sizes other than 4 or 8, instructions other than 4 bytes, or integral (non-float) numbers are refused, not read.
