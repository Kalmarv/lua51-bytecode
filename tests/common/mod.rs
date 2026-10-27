//! Builders for hand-made little-endian chunks (4-byte `int` and `size_t`).
#![allow(dead_code, clippy::indexing_slicing)]

pub const OP_LOADK: u32 = 1;
pub const OP_GETGLOBAL: u32 = 5;
pub const OP_GETTABLE: u32 = 6;
pub const OP_SETGLOBAL: u32 = 7;
pub const OP_CALL: u32 = 28;
/// `RETURN R0 B1`.
pub const RETURN: u32 = 30 | (1 << 23);

pub fn le(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

pub fn abx(op: u32, a: u32, bx: u32) -> u32 {
    op | (a << 6) | (bx << 14)
}

pub fn abc(op: u32, a: u32, b: u32, c: u32) -> u32 {
    op | (a << 6) | (c << 14) | (b << 23)
}

pub fn str_const(s: &[u8]) -> Vec<u8> {
    let mut out = vec![4];
    out.extend_from_slice(&le(u32::try_from(s.len() + 1).unwrap()));
    out.extend_from_slice(s);
    out.push(0);
    out
}

pub fn num_const(n: f64) -> Vec<u8> {
    let mut out = vec![3];
    out.extend_from_slice(&n.to_le_bytes());
    out
}

/// The header of a little-endian chunk, then `main`.
pub fn chunk(main: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1b, b'L', b'u', b'a', 0x51, 0, 1, 4, 4, 4, 8, 0];
    out.extend_from_slice(main);
    out
}

/// A function with no source name, no upvalues and no debug information.
pub fn func(words: &[u32], consts: &[Vec<u8>], max_stack: u8) -> Vec<u8> {
    func_with(words, consts, max_stack, &[], &[0; 12])
}

/// A function with its nested functions and its raw debug tables (line, local and upvalue-name counts and bodies).
pub fn func_with(
    words: &[u32],
    consts: &[Vec<u8>],
    max_stack: u8,
    nested: &[Vec<u8>],
    debug: &[u8],
) -> Vec<u8> {
    let mut out = vec![0; 4 + 8];
    out.extend_from_slice(&[0, 0, 2, max_stack]);
    out.extend_from_slice(&le(u32::try_from(words.len()).unwrap()));
    for w in words {
        out.extend_from_slice(&le(*w));
    }
    out.extend_from_slice(&le(u32::try_from(consts.len()).unwrap()));
    for c in consts {
        out.extend_from_slice(c);
    }
    out.extend_from_slice(&le(u32::try_from(nested.len()).unwrap()));
    for n in nested {
        out.extend_from_slice(n);
    }
    out.extend_from_slice(debug);
    out
}
