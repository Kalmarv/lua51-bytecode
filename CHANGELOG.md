# Changelog

## 0.2.0 (2026-10-29)

- Fix: a `SETLIST` with `C` 0 (`luac` writes it for tables of more than 25,550 items) is followed by a word that is the block number, which was refused as an unknown opcode when its low six bits made one. It is now read as an `Op::Data` instruction (breaking: `Op` gains the variant `Data`, and `Instruction` a `word` field and a `data` constructor), and a `SETLIST` with `C` 0 at the end of the code is refused.
- Tests: the 271 test sources of unluac that a stock Lua 5.1.5 `luac` compiles, with their 542 chunks (with and without debug information), are parsed and checked (`tests/unluac_corpus.rs`).

## 0.1.1 (2026-10-27)

- README: install from crates.io (0.1.0's README still said to depend on the repository). No code changes.

## 0.1.0 (2026-10-27)

- First release: Lua 5.1 precompiled bytecode (luac output, either byte order), and a lister of the calls made with constant arguments.
