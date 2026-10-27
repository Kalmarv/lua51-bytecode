//! The calls a function makes whose callee and arguments are known without running it.
//!
//! A configuration script is a list of calls of engine functions with literals: `change_fog(400, 700, -10)`,
//! `level.LoadData(level, "path")`. [`calls`] follows each function's registers through the instructions that put
//! constants, globals and fields of them in place (`LOADK`, `LOADBOOL`, `LOADNIL`, `MOVE`, `GETGLOBAL`, `GETTABLE`,
//! `SELF`, arithmetic on numbers) and reports every `CALL` and `TAILCALL` with what its callee and arguments held.
//! What it does not know is [`Value::Unknown`]: it does not run branches or loops, so every instruction a jump can
//! reach starts from nothing known, and a value an upvalue, a call's result or any other operation made is unknown.
//! A call under a condition is listed as the unconditional call it would be, with its arguments as known at that
//! point on the straight path.
#![deny(clippy::disallowed_methods, clippy::disallowed_types)]
#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]

use crate::{Constant, Instruction, Layout, Op, Proto};
use std::sync::Arc;

/// The most nodes a [`Value::Field`] chain may hold (every field, its table and its key count one each); a deeper
/// or wider value is [`Value::Unknown`]. The bound keeps a hostile chain of `GETTABLE`s from building values that
/// are thousands deep or that double with every instruction.
pub const MAX_VALUE_NODES: usize = 64;

/// What a register held when a call read it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// Not known without running the function (also a field chain of more than [`MAX_VALUE_NODES`] nodes).
    Unknown,
    /// `nil`.
    Nil,
    /// A boolean (`LOADBOOL`, or a boolean constant).
    Bool(bool),
    /// A number: a numeric constant or arithmetic on constant numbers (folded as the VM would, in `f64`).
    Number(f64),
    /// A string constant, shared with every register and call that read it.
    Str(Arc<[u8]>),
    /// The global of that name.
    Global(Arc<[u8]>),
    /// `table[key]` (a field, or a method for `SELF`), both known.
    Field(Box<Value>, Box<Value>),
    /// The closure of the function at that index of the enclosing function's nested functions.
    Closure(usize),
    /// What the call at that instruction of the same function returned.
    Result(usize),
}

/// A call and what it read.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    /// Which function made it: the indices of the nested functions from the main function down (empty: the main one).
    pub path: Vec<usize>,
    /// The index of its `CALL` or `TAILCALL`.
    pub pc: usize,
    /// Its source line; `None` in a stripped chunk.
    pub line: Option<u32>,
    /// What the called register held: usually a [`Value::Global`] or a [`Value::Field`] of one.
    pub callee: Value,
    /// The arguments (for a method call by `SELF`, the object first). Empty when `open_args`.
    pub args: Vec<Value>,
    /// The argument list runs to the stack top (the last argument is a call or `...`): its length is not known.
    pub open_args: bool,
    /// The instruction is a `TAILCALL` (`return f(...)`), not a `CALL`.
    pub tail: bool,
}

/// Every call in `root` and its nested functions, the function's own first, in instruction order.
///
/// The list holds one [`Call`] per `CALL` or `TAILCALL` instruction, each with at most 510 arguments, and the values
/// share the strings of the constant tables: the memory used is bounded by the code and the constants of `root`.
pub fn calls(root: &Proto) -> Vec<Call> {
    let mut out = Vec::new();
    walk(root, &mut Vec::new(), &mut out);
    out
}

fn walk(p: &Proto, path: &mut Vec<usize>, out: &mut Vec<Call>) {
    let consts: Vec<Value> = p
        .constants
        .iter()
        .map(|c| match c {
            Constant::Nil => Value::Nil,
            Constant::Bool(b) => Value::Bool(*b),
            Constant::Number(n) => Value::Number(*n),
            Constant::Str(s) => Value::Str(Arc::from(s.as_slice())),
        })
        .collect();
    let mut targets = vec![false; p.code.len()];
    let mut mark = |t: Option<usize>| {
        if let Some(slot) = t.and_then(|t| targets.get_mut(t)) {
            *slot = true;
        }
    };
    for (pc, i) in p.code.iter().enumerate() {
        let skip = pc.checked_add(2);
        match i.op {
            Op::Jmp | Op::ForLoop | Op::ForPrep => mark(jump(pc, i)),
            Op::LoadBool if i.c != 0 => mark(skip),
            op if op.tests() => mark(skip),
            _ => {}
        }
    }

    let mut regs = vec![Value::Unknown; usize::from(p.max_stack).saturating_add(1)];
    let mut pc = 0;
    while let Some(i) = p.code.get(pc) {
        if targets.get(pc) == Some(&true) {
            regs.fill(Value::Unknown);
        }
        let a = usize::from(i.a);
        match i.op {
            Op::Move => {
                let v = get(&regs, usize::from(i.b));
                put(&mut regs, a, v);
            }
            Op::LoadK => put(&mut regs, a, constant(&consts, usize_of(i.bx()))),
            Op::LoadBool => put(&mut regs, a, Value::Bool(i.b != 0)),
            Op::LoadNil => {
                for r in a..=usize::from(i.b) {
                    put(&mut regs, r, Value::Nil);
                }
            }
            Op::GetGlobal => {
                let v = match constant(&consts, usize_of(i.bx())) {
                    Value::Str(name) => Value::Global(name),
                    _ => Value::Unknown,
                };
                put(&mut regs, a, v);
            }
            Op::GetTable => {
                let v = field(get(&regs, usize::from(i.b)), rk(&consts, &regs, i.c));
                put(&mut regs, a, v);
            }
            Op::SelfOp => {
                let object = get(&regs, usize::from(i.b));
                let v = field(object.clone(), rk(&consts, &regs, i.c));
                put(&mut regs, a.saturating_add(1), object);
                put(&mut regs, a, v);
            }
            Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod | Op::Pow => {
                let v = match (rk(&consts, &regs, i.b), rk(&consts, &regs, i.c)) {
                    (Value::Number(x), Value::Number(y)) => Value::Number(arithmetic(i.op, x, y)),
                    _ => Value::Unknown,
                };
                put(&mut regs, a, v);
            }
            Op::Unm => {
                let v = match get(&regs, usize::from(i.b)) {
                    Value::Number(x) => Value::Number(-x),
                    _ => Value::Unknown,
                };
                put(&mut regs, a, v);
            }
            Op::Closure => {
                put(&mut regs, a, Value::Closure(usize_of(i.bx())));
                // the pseudo-instructions that follow capture upvalues; they are not MOVEs and GETUPVALs of this function
                let ups = p
                    .protos
                    .get(usize_of(i.bx()))
                    .map_or(0, |c| usize::from(c.upvalues));
                pc = pc.saturating_add(ups);
            }
            Op::Call | Op::TailCall => {
                let open = i.b == 0;
                let args = if open {
                    Vec::new()
                } else {
                    (1..usize::from(i.b))
                        .map(|k| get(&regs, a.saturating_add(k)))
                        .collect()
                };
                out.push(Call {
                    path: path.clone(),
                    pc,
                    line: p.lines.get(pc).copied(),
                    callee: get(&regs, a),
                    args,
                    open_args: open,
                    tail: i.op == Op::TailCall,
                });
                forget_from(&mut regs, a);
                if i.op == Op::Call {
                    put(&mut regs, a, Value::Result(pc));
                }
            }
            Op::VarArg | Op::ForLoop | Op::ForPrep | Op::TForLoop => forget_from(&mut regs, a),
            Op::GetUpval | Op::NewTable | Op::Not | Op::Len | Op::Concat | Op::TestSet => {
                put(&mut regs, a, Value::Unknown);
            }
            Op::SetGlobal
            | Op::SetUpval
            | Op::SetTable
            | Op::Jmp
            | Op::Eq
            | Op::Lt
            | Op::Le
            | Op::Return
            | Op::SetList
            | Op::Close
            | Op::Test => {}
        }
        pc = pc.saturating_add(1);
    }

    for (k, child) in p.protos.iter().enumerate() {
        path.push(k);
        walk(child, path, out);
        path.pop();
    }
}

/// The instruction a `JMP`, `FORLOOP` or `FORPREP` at `pc` goes to.
fn jump(pc: usize, i: &Instruction) -> Option<usize> {
    debug_assert_eq!(i.op.layout(), Layout::AsBx);
    let t = i64::try_from(pc).ok()?.checked_add(1)?.checked_add(i.sbx())?;
    usize::try_from(t).ok()
}

fn usize_of(x: u32) -> usize {
    usize::try_from(x).unwrap_or(usize::MAX)
}

fn get(regs: &[Value], r: usize) -> Value {
    regs.get(r).cloned().unwrap_or(Value::Unknown)
}

fn put(regs: &mut [Value], r: usize, v: Value) {
    if let Some(slot) = regs.get_mut(r) {
        *slot = v;
    }
}

fn forget_from(regs: &mut [Value], r: usize) {
    for slot in regs.iter_mut().skip(r) {
        *slot = Value::Unknown;
    }
}

fn constant(consts: &[Value], k: usize) -> Value {
    consts.get(k).cloned().unwrap_or(Value::Unknown)
}

/// A register-or-constant operand.
fn rk(consts: &[Value], regs: &[Value], x: u16) -> Value {
    match usize::from(x).checked_sub(256) {
        Some(k) => constant(consts, k),
        None => get(regs, usize::from(x)),
    }
}

fn field(table: Value, key: Value) -> Value {
    let nodes = nodes(&table).saturating_add(nodes(&key)).saturating_add(1);
    if table == Value::Unknown || key == Value::Unknown || nodes > MAX_VALUE_NODES {
        Value::Unknown
    } else {
        Value::Field(Box::new(table), Box::new(key))
    }
}

/// How many nodes a value holds (at most [`MAX_VALUE_NODES`], as `field` allows no more).
fn nodes(v: &Value) -> usize {
    match v {
        Value::Field(table, key) => nodes(table).saturating_add(nodes(key)).saturating_add(1),
        _ => 1,
    }
}

/// Lua 5.1's arithmetic on numbers (`luai_nummod` is `a - floor(a / b) * b`).
fn arithmetic(op: Op, x: f64, y: f64) -> f64 {
    match op {
        Op::Add => x + y,
        Op::Sub => x - y,
        Op::Mul => x * y,
        Op::Div => x / y,
        Op::Mod => x - (x / y).floor() * y,
        _ => x.powf(y),
    }
}
