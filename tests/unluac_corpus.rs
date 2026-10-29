//! Every chunk a stock `luac` 5.1.5 made from unluac's test sources (see `tests/fixtures/unluac/README.txt`) reads,
//! and a function's stripped and unstripped chunks agree on everything but the debug information.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use lua51_bytecode::{Chunk, Endian, Proto};
use std::path::Path;

const SOURCES: usize = 271;
/// Functions over all the chunks (361 in each of the two sets).
const FUNCTIONS: usize = 722;

fn dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/unluac"))
}

/// The names (without extension) of the sources that have fixtures, sorted.
fn names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir())
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            name.strip_suffix(".lua").map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

fn read(name: &str, suffix: &str) -> Chunk {
    let path = dir().join(format!("{name}.{suffix}"));
    let bytes = std::fs::read(&path).unwrap();
    Chunk::parse(&bytes).unwrap_or_else(|e| panic!("{name}.{suffix}: {e}"))
}

/// `a` and `b` are the same function, `b` without its debug information.
fn same_but_stripped(a: &Proto, b: &Proto, at: &str) {
    assert_eq!(a.code, b.code, "{at}: code");
    assert_eq!(a.constants, b.constants, "{at}: constants");
    assert_eq!(
        (a.upvalues, a.params, a.vararg, a.max_stack),
        (b.upvalues, b.params, b.vararg, b.max_stack),
        "{at}: signature"
    );
    assert_eq!(a.protos.len(), b.protos.len(), "{at}: nested functions");
    assert!(b.source.is_none(), "{at}: stripped source name");
    assert!(
        b.lines.is_empty() && b.locals.is_empty() && b.upvalue_names.is_empty(),
        "{at}: stripped debug information"
    );
    assert_eq!(a.lines.len(), a.code.len(), "{at}: one line per instruction");
    for (i, (x, y)) in a.protos.iter().zip(&b.protos).enumerate() {
        same_but_stripped(x, y, &format!("{at}/{i}"));
    }
}

fn count(p: &Proto) -> usize {
    1 + p.protos.iter().map(count).sum::<usize>()
}

#[test]
fn every_chunk_reads() {
    let names = names();
    assert_eq!(names.len(), SOURCES);
    let (mut chunks, mut functions) = (0, 0);
    for name in &names {
        for suffix in ["luac", "s.luac"] {
            let chunk = read(name, suffix);
            assert_eq!(chunk.header.endian, Endian::Little, "{name}.{suffix}");
            assert_eq!((chunk.header.int_size, chunk.header.size_t_size), (4, 8));
            chunks += 1;
            functions += count(&chunk.main);
        }
    }
    assert_eq!(chunks, 2 * SOURCES);
    assert_eq!(chunks, 542);
    assert_eq!(functions, FUNCTIONS);
    // No fixture without a source, and no source without both chunks.
    let files = std::fs::read_dir(dir()).unwrap().count();
    assert_eq!(
        files,
        SOURCES * 3 + 2,
        "the sources, their chunks, LICENSE and README.txt"
    );
}

#[test]
fn stripped_chunks_differ_only_in_debug_information() {
    for name in names() {
        let full = read(&name, "luac");
        let stripped = read(&name, "s.luac");
        assert_eq!(full.header, stripped.header, "{name}");
        assert_eq!(
            full.main.source.as_deref(),
            Some(format!("@{name}.lua").as_bytes()),
            "{name}: source name"
        );
        same_but_stripped(&full.main, &stripped.main, &name);
    }
}
