# unluac test corpus, compiled with a stock luac 5.1.5

The `.lua` files are the test sources of unluac (<https://sourceforge.net/projects/unluac/>), MIT licensed; its
notice is in `LICENSE` here and applies to every `.lua` file in this folder. The `.luac` files are their compiled
chunks and are produced from those sources by the same notice.

Each source `X.lua` has two chunks, made with the unmodified Lua 5.1.5 `luac` (source tarball from lua.org, sha256
`2640fc56a795f29d28ef15e13c34a47e223960b0240e8cb0a82d9b0738695333`, built with MSVC for 64-bit Windows:
little-endian, 4-byte `int`, 8-byte `size_t`), run from this folder so the source name is `@X.lua`:

    luac -o X.luac X.lua        # with debug information
    luac -s -o X.s.luac X.lua   # stripped

unluac's test folder holds 283 sources. `luac` 5.1.5 compiles 271 of them (542 chunks). The 12 it refuses are left
out; they need syntax or limits of a later Lua:

- `52_goto01`, `52_goto02`, `52_goto03`, `52_goto04`, `52_goto05`, `52_goto06`, `52_goto08`: `goto` and labels (5.2)
- `52_loadkx01`: more constants than 5.1 can index ("constant table overflow"; it exists to test `LOADKX`, 5.2)
- `53_expression`, `53_expression02`: bitwise operators (5.3)
- `54_tbc01`: `<close>` attributes (5.4)
- `55_namedvararg01`: named vararg parameters (5.5)

`tests/unluac_corpus.rs` parses every chunk.
