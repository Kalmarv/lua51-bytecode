//! Lua 5.1 instructions: 32-bit words of a 6-bit opcode and `A` (8 bits), then `C` (9) and `B` (9), or `Bx` (18) over both.
#![deny(clippy::disallowed_methods, clippy::disallowed_types)]
#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]

/// The 38 opcodes of Lua 5.1, in `lopcodes.h`'s order (the number a word stores); `SelfOp` is `OP_SELF`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// `R(A) := R(B)`.
    Move,
    /// `R(A) := Kst(Bx)`: a constant into a register.
    LoadK,
    /// `R(A) := (bool)B`; skips the next instruction when `C` is not 0.
    LoadBool,
    /// `R(A) .. R(B) := nil`: every register from `A` to `B` inclusive.
    LoadNil,
    /// `R(A) := UpValue[B]`.
    GetUpval,
    /// `R(A) := Gbl[Kst(Bx)]`; the constant is the global's name.
    GetGlobal,
    /// `R(A) := R(B)[RK(C)]`.
    GetTable,
    /// `Gbl[Kst(Bx)] := R(A)`; the constant is the global's name.
    SetGlobal,
    /// `UpValue[B] := R(A)`.
    SetUpval,
    /// `R(A)[RK(B)] := RK(C)`.
    SetTable,
    /// `R(A) := {}`; `B` and `C` are the array and hash size hints, each in Lua's floating-point byte encoding.
    NewTable,
    /// `R(A+1) := R(B); R(A) := R(B)[RK(C)]`: a method call's object and function (`OP_SELF`).
    SelfOp,
    /// `R(A) := RK(B) + RK(C)`.
    Add,
    /// `R(A) := RK(B) - RK(C)`.
    Sub,
    /// `R(A) := RK(B) * RK(C)`.
    Mul,
    /// `R(A) := RK(B) / RK(C)`.
    Div,
    /// `R(A) := RK(B) % RK(C)`.
    Mod,
    /// `R(A) := RK(B) ^ RK(C)`.
    Pow,
    /// `R(A) := -R(B)`.
    Unm,
    /// `R(A) := not R(B)`.
    Not,
    /// `R(A) := length of R(B)`.
    Len,
    /// `R(A) := R(B) .. ... .. R(C)`: the registers `B` to `C` inclusive.
    Concat,
    /// `pc += sBx`.
    Jmp,
    /// `if (RK(B) == RK(C)) ~= A then pc++`; followed by a [`Op::Jmp`].
    Eq,
    /// `if (RK(B) < RK(C)) ~= A then pc++`; followed by a [`Op::Jmp`].
    Lt,
    /// `if (RK(B) <= RK(C)) ~= A then pc++`; followed by a [`Op::Jmp`].
    Le,
    /// `if not (R(A) <=> C) then pc++`: skips the next instruction unless the truth of `R(A)` is `C`; followed by a [`Op::Jmp`].
    Test,
    /// `if (R(B) <=> C) then R(A) := R(B) else pc++`; followed by a [`Op::Jmp`].
    TestSet,
    /// `R(A) .. R(A+C-2) := R(A)(R(A+1) .. R(A+B-1))`; `B` 0 takes arguments up to the stack top, `C` 0 keeps all results.
    Call,
    /// `return R(A)(R(A+1) .. R(A+B-1))`; `B` 0 takes arguments up to the stack top.
    TailCall,
    /// `return R(A) .. R(A+B-2)`; `B` 0 returns up to the stack top.
    Return,
    /// Numeric `for` step: `R(A) += R(A+2)`, and while `R(A)` has not passed the limit `R(A+1)` it sets `R(A+3) := R(A)` and jumps by `sBx`.
    ForLoop,
    /// Numeric `for` start: `R(A) -= R(A+2); pc += sBx`.
    ForPrep,
    /// Generic `for` step: `R(A+3) .. R(A+2+C) := R(A)(R(A+1), R(A+2))`; if `R(A+3)` is not nil it becomes `R(A+2)`, otherwise the next instruction (a [`Op::Jmp`] back) is skipped.
    TForLoop,
    /// `R(A)[(C-1)*50 + i] := R(A+i)` for `1 <= i <= B`: stores up to 50 table items per instruction; `B` 0 takes up to the stack top.
    SetList,
    /// Closes the upvalues of every local from `R(A)` up.
    Close,
    /// `R(A) := closure(KPROTO[Bx])`; followed by one `MOVE` or `GETUPVAL` instruction per upvalue of that function.
    Closure,
    /// `R(A) .. R(A+B-2) = ...`; `B` 0 takes all the extra arguments.
    VarArg,
}

const OPS: [Op; 38] = [
    Op::Move,
    Op::LoadK,
    Op::LoadBool,
    Op::LoadNil,
    Op::GetUpval,
    Op::GetGlobal,
    Op::GetTable,
    Op::SetGlobal,
    Op::SetUpval,
    Op::SetTable,
    Op::NewTable,
    Op::SelfOp,
    Op::Add,
    Op::Sub,
    Op::Mul,
    Op::Div,
    Op::Mod,
    Op::Pow,
    Op::Unm,
    Op::Not,
    Op::Len,
    Op::Concat,
    Op::Jmp,
    Op::Eq,
    Op::Lt,
    Op::Le,
    Op::Test,
    Op::TestSet,
    Op::Call,
    Op::TailCall,
    Op::Return,
    Op::ForLoop,
    Op::ForPrep,
    Op::TForLoop,
    Op::SetList,
    Op::Close,
    Op::Closure,
    Op::VarArg,
];

/// What an operand holds (`lopcodes.c`): `Unused`; a plain number (`Number`); a register (`Register`); a register or,
/// from 256 on, a constant (`RegisterOrConstant`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operand {
    /// The field is not used.
    Unused,
    /// A plain number: a count, a flag, an index into a table of upvalues or a size hint.
    Number,
    /// A register number.
    Register,
    /// A register number below 256, or a constant index from 256 on (`256 + index`).
    RegisterOrConstant,
}

/// How an instruction lays out its operands after `A`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// `B` and `C`.
    Abc,
    /// `Bx`, an unsigned 18-bit number.
    Abx,
    /// `sBx`, `Bx` less 131071.
    AsBx,
}

impl Op {
    /// The opcode a word stores in its low 6 bits; `None` past the 38 that exist.
    pub fn from_code(code: u8) -> Option<Op> {
        OPS.get(usize::from(code)).copied()
    }

    /// Whether the instruction sets register `A` (`lopcodes.c`'s `testAMode`).
    pub fn sets_a(self) -> bool {
        !matches!(
            self,
            Op::SetGlobal
                | Op::SetUpval
                | Op::SetTable
                | Op::Jmp
                | Op::Eq
                | Op::Lt
                | Op::Le
                | Op::Return
                | Op::TForLoop
                | Op::SetList
                | Op::Close
        )
    }

    /// Whether the next instruction is a skipped jump: `Eq`, `Lt`, `Le`, `Test`, `TestSet` and `TForLoop` each either
    /// run the instruction after them (a `Jmp`) or skip it.
    pub fn tests(self) -> bool {
        matches!(
            self,
            Op::Eq | Op::Lt | Op::Le | Op::Test | Op::TestSet | Op::TForLoop
        )
    }

    /// How the instruction lays out its operands after `A`.
    pub fn layout(self) -> Layout {
        match self {
            Op::LoadK | Op::GetGlobal | Op::SetGlobal | Op::Closure => Layout::Abx,
            Op::Jmp | Op::ForLoop | Op::ForPrep => Layout::AsBx,
            _ => Layout::Abc,
        }
    }

    /// What `B` holds (meaningful for [`Layout::Abc`]).
    pub fn b_operand(self) -> Operand {
        match self {
            Op::Move | Op::LoadNil | Op::GetTable | Op::SelfOp | Op::Unm | Op::Not | Op::Len | Op::Concat => {
                Operand::Register
            }
            Op::SetTable
            | Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Mod
            | Op::Pow
            | Op::Eq
            | Op::Lt
            | Op::Le => Operand::RegisterOrConstant,
            Op::LoadBool
            | Op::GetUpval
            | Op::SetUpval
            | Op::NewTable
            | Op::Call
            | Op::TailCall
            | Op::Return
            | Op::VarArg => Operand::Number,
            Op::Test | Op::TestSet => Operand::Register,
            _ => Operand::Unused,
        }
    }

    /// What `C` holds (meaningful for [`Layout::Abc`]).
    pub fn c_operand(self) -> Operand {
        match self {
            Op::GetTable
            | Op::SetTable
            | Op::SelfOp
            | Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Mod
            | Op::Pow
            | Op::Eq
            | Op::Lt
            | Op::Le => Operand::RegisterOrConstant,
            Op::Concat => Operand::Register,
            Op::LoadBool
            | Op::NewTable
            | Op::Test
            | Op::TestSet
            | Op::Call
            | Op::TailCall
            | Op::TForLoop
            | Op::SetList => Operand::Number,
            _ => Operand::Unused,
        }
    }
}

/// A decoded instruction. `b` and `c` are the 9-bit fields, `bx` the 18-bit one over both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instruction {
    /// The opcode.
    pub op: Op,
    /// The 8-bit `A` field.
    pub a: u8,
    /// The 9-bit `B` field (see [`Instruction::bx`] for the layouts that join it with `C`).
    pub b: u16,
    /// The 9-bit `C` field.
    pub c: u16,
}

/// The low `N` bytes of a number whose value is known to fit them.
fn low<const N: usize>(v: u32) -> [u8; N] {
    let bytes = v.to_le_bytes();
    let mut out = [0; N];
    for (o, b) in out.iter_mut().zip(bytes) {
        *o = b;
    }
    out
}

impl Instruction {
    /// Decode a word; `Err` is its opcode when no such opcode exists.
    pub fn decode(word: u32) -> Result<Instruction, u8> {
        let [code] = low::<1>(word & 0x3f);
        let op = Op::from_code(code).ok_or(code)?;
        let [a] = low::<1>((word >> 6) & 0xff);
        Ok(Instruction {
            op,
            a,
            c: u16::from_le_bytes(low::<2>((word >> 14) & 0x1ff)),
            b: u16::from_le_bytes(low::<2>((word >> 23) & 0x1ff)),
        })
    }

    /// `Bx`: `B` and `C` as one unsigned 18-bit number.
    pub fn bx(self) -> u32 {
        (u32::from(self.b) << 9) | u32::from(self.c)
    }

    /// `sBx`: `Bx` less 131071 (a jump's offset from the next instruction).
    pub fn sbx(self) -> i64 {
        i64::from(self.bx()).saturating_sub(131_071)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `GETGLOBAL R2 K3` is opcode 5, A 2, Bx 3: 5 | 2 << 6 | 3 << 14.
    #[test]
    fn abx_word() {
        let i = Instruction::decode(0x0000_C085).unwrap();
        assert_eq!((i.op, i.a, i.bx()), (Op::GetGlobal, 2, 3));
    }

    /// `CALL R1 B3 C2`: opcode 28, A 1, C 2 << 14, B 3 << 23.
    #[test]
    fn abc_word() {
        let i = Instruction::decode(0x0180_805C).unwrap();
        assert_eq!((i.op, i.a, i.b, i.c), (Op::Call, 1, 3, 2));
    }

    /// `JMP` of -1 stores Bx 131070 = 0x1FFFE: 22 | 0x1FFFE << 14.
    #[test]
    fn signed_jump() {
        let i = Instruction::decode(0x7FFF_8016).unwrap();
        assert_eq!((i.op, i.sbx()), (Op::Jmp, -1));
    }

    /// Opcode 38 (`6 bits` allow up to 63) does not exist.
    #[test]
    fn unknown_opcode() {
        assert_eq!(Instruction::decode(38), Err(38));
        assert_eq!(Instruction::decode(63), Err(63));
        assert!(Instruction::decode(37).is_ok());
    }
}
