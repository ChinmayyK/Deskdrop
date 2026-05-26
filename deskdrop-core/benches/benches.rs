// Deskdrop benchmarks
//
// Run:  cargo bench
// HTML: target/criterion/report/index.html

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use deskdrop_core::{
    chunked::{maybe_chunk, Reassembler},
    crypto::EphemeralKeypair,
    dedup::{hash_content, Deduplicator},
    protocol::ClipboardContent,
};

// ── Crypto benchmarks ─────────────────────────────────────────────────────────

fn bench_handshake(c: &mut Criterion) {
    c.bench_function("x25519_ecdh_hkdf", |b| {
        b.iter(|| {
            let alice = EphemeralKeypair::generate();
            let bob = EphemeralKeypair::generate();
            let bob_pub = bob.public_bytes;
            let _alice_sess = alice.derive_session_key(black_box(bob_pub)).unwrap();
        })
    });
}

fn bench_encryption(c: &mut Criterion) {
    let mut group = c.benchmark_group("chacha20_poly1305");

    for size in [1_024usize, 64_000, 1_024_000, 4_096_000] {
        let payload = vec![0u8; size];
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("encrypt", size), &payload, |b, payload| {
            let alice = EphemeralKeypair::generate();
            let bob = EphemeralKeypair::generate();
            let bob_pub = bob.public_bytes;
            let mut sess = alice.derive_session_key(bob_pub).unwrap();
            b.iter(|| sess.encrypt(black_box(payload)).unwrap())
        });

        group.bench_with_input(
            BenchmarkId::new("encrypt_decrypt", size),
            &payload,
            |b, payload| {
                let alice = EphemeralKeypair::generate();
                let bob = EphemeralKeypair::generate();
                let _a_pub = alice.public_bytes;
                let b_pub = bob.public_bytes;
                let _send = alice.derive_session_key(b_pub).unwrap();
                let alice2 = EphemeralKeypair::generate();
                let bob2 = EphemeralKeypair::generate();
                let a2_pub = alice2.public_bytes;
                let b2_pub = bob2.public_bytes;
                let mut recv = bob2.derive_session_key(a2_pub).unwrap();
                let mut send2 = alice2.derive_session_key(b2_pub).unwrap();

                b.iter(|| {
                    let ct = send2.encrypt(black_box(payload)).unwrap();
                    let _ = recv.decrypt(black_box(&ct)).unwrap();
                })
            },
        );
    }
    group.finish();
}

// ── Content hash ──────────────────────────────────────────────────────────────

fn bench_content_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("content_hash_sha256");
    for size in [256usize, 8_192, 65_536, 1_048_576] {
        let content = ClipboardContent::Text("A".repeat(size));
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &content, |b, c| {
            b.iter(|| hash_content(black_box(c)))
        });
    }
    group.finish();
}

// ── Chunked transfer ──────────────────────────────────────────────────────────

fn bench_chunk_and_reassemble(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunked_transfer");

    for mb in [1u64, 4, 16, 32] {
        let size = (mb * 1024 * 1024) as usize;
        let content = ClipboardContent::Image {
            mime: "image/png".into(),
            data: vec![0xAB; size],
        };
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(
            BenchmarkId::new("chunk_and_reassemble_MB", mb),
            &content,
            |b, content| {
                b.iter(|| {
                    let msgs = maybe_chunk(black_box(content)).unwrap();
                    let mut r = Reassembler::default();
                    for msg in msgs {
                        r.feed(msg).unwrap();
                    }
                })
            },
        );
    }
    group.finish();
}

// ── Deduplication ─────────────────────────────────────────────────────────────

fn bench_dedup(c: &mut Criterion) {
    c.bench_function("dedup_should_apply", |b| {
        let mut dedup = Deduplicator::new();
        let content = ClipboardContent::Text("hello world".into());
        let hash = hash_content(&content);
        b.iter(|| dedup.should_apply(uuid::Uuid::new_v4(), black_box(hash)))
    });
}

criterion_group!(
    benches,
    bench_handshake,
    bench_encryption,
    bench_content_hash,
    bench_chunk_and_reassemble,
    bench_dedup,
);
criterion_main!(benches);
