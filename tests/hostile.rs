//! Chunks built by hand that a careless reader would panic on, over-allocate for, or report at the wrong byte.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use common::*;
use lua51_bytecode::{Chunk, Op, Value, calls};

fn parse_err(bytes: &[u8]) -> lua51_bytecode::Error {
    Chunk::parse(bytes).unwrap_err()
}

/// A local variable record whose `end_pc` is past the code is reported at the `end_pc` field, not at the record.
#[test]
fn local_range_error_names_the_bad_field() {
    let local = |start: u32, end: u32| {
        let mut debug = vec![0, 0, 0, 0]; // no line numbers
        debug.extend_from_slice(&le(1)); // one local
        debug.extend_from_slice(&le(2));
        debug.extend_from_slice(b"x\0");
        debug.extend_from_slice(&le(start));
        debug.extend_from_slice(&le(end));
        debug.extend_from_slice(&le(0)); // no upvalue names
        chunk(&func_with(&[RETURN], &[], 2, &[], &debug))
    };
    let bytes = local(0, 5);
    let e = parse_err(&bytes);
    assert_eq!(e.offset, bytes.len() - 8, "{e}");
    assert!(
        e.detail.contains("end_pc 5") && e.detail.contains("at most 1"),
        "{e}"
    );

    let bytes = local(1, 0);
    let e = parse_err(&bytes);
    assert_eq!(e.offset, bytes.len() - 12, "{e}");
    assert!(
        e.detail.contains("start_pc 1") && e.detail.contains("end_pc 0"),
        "{e}"
    );
}

/// `GETGLOBAL` and `SETGLOBAL` name their global with a string constant (`luaG_checkcode`).
#[test]
fn global_access_needs_a_string_constant() {
    for op in [OP_GETGLOBAL, OP_SETGLOBAL] {
        let bytes = chunk(&func(&[abx(op, 0, 0), RETURN], &[num_const(1.5)], 2));
        let e = parse_err(&bytes);
        assert_eq!(e.offset, 12 + 4 + 8 + 4 + 4, "{e}");
        assert!(e.detail.contains("string"), "{e}");
        let ok = chunk(&func(&[abx(op, 0, 0), RETURN], &[str_const(b"g")], 2));
        Chunk::parse(&ok).unwrap();
    }
}

/// The number-size byte (10) and the integral-flag byte (11) are separate fields.
#[test]
fn integral_flag_is_reported_at_its_own_byte() {
    let mut bytes = chunk(&func(&[RETURN], &[], 2));
    bytes[11] = 1;
    let e = parse_err(&bytes);
    assert_eq!(e.offset, 11, "{e}");
    assert!(e.detail.contains("integral"), "{e}");
    bytes[11] = 0;
    bytes[10] = 4;
    assert_eq!(parse_err(&bytes).offset, 10);
}

/// The count of line numbers and of upvalue names are reported where the count is, with what was expected.
#[test]
fn debug_count_errors_say_what_was_expected() {
    // two line numbers for one instruction
    let mut debug = vec![];
    debug.extend_from_slice(&le(2));
    debug.extend_from_slice(&le(1));
    debug.extend_from_slice(&le(1));
    debug.extend_from_slice(&le(0));
    debug.extend_from_slice(&le(0));
    let bytes = chunk(&func_with(&[RETURN], &[], 2, &[], &debug));
    let e = parse_err(&bytes);
    assert_eq!(e.offset, bytes.len() - 20, "{e}");
    assert!(e.detail.contains("expected 0 or 1"), "{e}");

    // one upvalue name for a function without upvalues
    let mut debug = vec![];
    debug.extend_from_slice(&le(0));
    debug.extend_from_slice(&le(0));
    debug.extend_from_slice(&le(1));
    debug.extend_from_slice(&le(2));
    debug.extend_from_slice(b"u\0");
    let bytes = chunk(&func_with(&[RETURN], &[], 2, &[], &debug));
    let e = parse_err(&bytes);
    assert_eq!(e.offset, bytes.len() - 10, "{e}");
    assert!(e.detail.contains("expected 0 or 0"), "{e}");
}

/// A chain of field accesses is bounded: past the cap the value is unknown, not a structure thousands deep.
#[test]
fn deep_field_chains_become_unknown() {
    let consts = [str_const(b"a"), str_const(b"k")];
    let call_after = |steps: usize| {
        let mut words = vec![abx(OP_GETGLOBAL, 0, 0)];
        // R0 = R0["k"]
        words.extend(std::iter::repeat_n(abc(OP_GETTABLE, 0, 0, 257), steps));
        words.push(abc(OP_CALL, 0, 1, 1));
        words.push(RETURN);
        let c = Chunk::parse(&chunk(&func(&words, &consts, 2))).unwrap();
        calls(&c.main).remove(0).callee
    };
    assert!(matches!(call_after(3), Value::Field(..)));
    assert_eq!(call_after(5000), Value::Unknown);
}

/// `R0 = R0[R0]` doubles the value each time; the lister must not build 2^N nodes.
#[test]
fn self_referential_fields_do_not_double_without_bound() {
    let consts = [str_const(b"a")];
    let mut words = vec![abx(OP_GETGLOBAL, 0, 0)];
    words.extend(std::iter::repeat_n(abc(OP_GETTABLE, 0, 0, 0), 60));
    words.push(abc(OP_CALL, 0, 1, 1));
    words.push(RETURN);
    let c = Chunk::parse(&chunk(&func(&words, &consts, 2))).unwrap();
    assert_eq!(calls(&c.main)[0].callee, Value::Unknown);
}

/// `SETLIST` with `C` 0 is followed by a word that is the block number, not an instruction (`lopcodes.h`): it can
/// hold any bits, takes a pc, and `luac` writes it for tables of more than 25,550 items.
#[test]
fn set_list_with_c_0_takes_a_data_word() {
    const OP_SETLIST: u32 = 34;
    let set_list = abc(OP_SETLIST, 0, 3, 0);
    // 0x226 is what luac wrote after such a SETLIST in a real chunk; its low six bits make opcode 38, which no
    // instruction has.
    for number in [0x226, 0xFFFF_FFFF, 0] {
        let bytes = chunk(&func(&[set_list, number, RETURN], &[], 4));
        let c = Chunk::parse(&bytes).unwrap();
        let ops: Vec<_> = c.main.code.iter().map(|i| i.op).collect();
        assert_eq!(ops, [Op::SetList, Op::Data, Op::Return]);
        assert_eq!(c.main.code[1].word, number);
    }
    // The same word after a SETLIST with C 1 is an instruction, and 0x226 is no instruction.
    let bytes = chunk(&func(&[abc(OP_SETLIST, 0, 3, 1), 0x226, RETURN], &[], 4));
    assert!(parse_err(&bytes).detail.contains("opcode 38"));
    // A SETLIST with C 0 that ends the code has no block number.
    let bytes = chunk(&func(&[RETURN, set_list], &[], 4));
    assert!(parse_err(&bytes).detail.contains("no block number"));
}
