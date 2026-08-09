use dhat::{Dhat, DhatAlloc};
use deskdrop_core::chunked::{maybe_chunk, Reassembler};
use deskdrop_core::protocol::ClipboardContent;

#[global_allocator]
static ALLOC: DhatAlloc = DhatAlloc;

fn main() {
    let _profiler = Dhat::start_heap_profiling();

    println!("Benchmarking 200MB Transfer Allocations...");
    let size = 200 * 1024 * 1024; // 200 MB
    let content = ClipboardContent::Image {
        mime: "image/png".into(),
        data: vec![0xAB; size], // Simulate large byte vector
    };

    // Simulate sending chunked payloads
    let msgs = maybe_chunk(&content).unwrap();

    let mut r = Reassembler::default();
    for msg in msgs {
        r.feed(msg).unwrap();
    }

    println!("Reassembly complete. Profiler will output dhat-heap.json on exit.");
}
