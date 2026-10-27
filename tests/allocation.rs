//! What the reader and the lister allocate for hostile chunks, measured with a counting allocator.
#![allow(unsafe_code, clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use common::*;
use lua51_bytecode::{Chunk, calls};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call is forwarded unchanged to the system allocator; the counters are only bookkeeping.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let now = CURRENT.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
        PEAK.fetch_max(now, Ordering::SeqCst);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size(), Ordering::SeqCst);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// The most memory live at once above what was live when `f` started.
fn peak_of<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let base = CURRENT.load(Ordering::SeqCst);
    PEAK.store(base, Ordering::SeqCst);
    let out = f();
    (out, PEAK.load(Ordering::SeqCst).saturating_sub(base))
}

/// One test, so nothing else allocates while a peak is measured.
#[test]
fn allocations_follow_the_bytes_present() {
    // A count that the remaining bytes can hold only if every item is one byte: with the real least size of a
    // nested function, or of a local, the count is refused before anything is allocated for it.
    let junk = vec![0u8; 1 << 20];
    let count = u32::try_from(junk.len()).unwrap();
    let mut bytes = chunk(&func(&[RETURN], &[], 2));
    // the main function's count of nested functions is the third count after the code: rewrite it
    let at = bytes.len() - 12 - 4;
    bytes[at..at + 4].copy_from_slice(&le(count));
    bytes.truncate(at + 4);
    bytes.extend_from_slice(&junk);
    let (r, peak) = peak_of(|| Chunk::parse(&bytes));
    let e = r.unwrap_err();
    assert!(
        peak < 4 << 20,
        "{peak} bytes allocated for a count of {count} functions: {e}"
    );
    assert_eq!(e.offset, at, "{e}");

    let mut bytes = chunk(&func(&[RETURN], &[], 2));
    let at = bytes.len() - 8;
    bytes[at..at + 4].copy_from_slice(&le(count / 4));
    bytes.truncate(at + 4);
    bytes.extend_from_slice(&junk);
    let (r, peak) = peak_of(|| Chunk::parse(&bytes));
    let e = r.unwrap_err();
    assert!(peak < 4 << 20, "{peak} bytes allocated for local variables: {e}");

    // A string constant read into 200 registers and passed to 50 calls is stored once.
    let big = vec![b'x'; 20_000];
    let mut words: Vec<u32> = (1..=200).map(|r| abx(OP_LOADK, r, 0)).collect();
    words.push(abc(OP_CALL, 0, 201, 1));
    let body: Vec<u32> = std::iter::repeat_n(words, 50).flatten().collect();
    let mut words = body;
    words.push(RETURN);
    let c = Chunk::parse(&chunk(&func(&words, &[str_const(&big)], 201))).unwrap();
    let (list, peak) = peak_of(|| calls(&c.main));
    assert_eq!(list.len(), 50);
    assert!(
        peak < 8 << 20,
        "{peak} bytes for 50 calls that name one 20 KB string"
    );
}
