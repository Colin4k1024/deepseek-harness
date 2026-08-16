//! Memory benchmarks — store, search, semantic search.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use dsh_core_memory::{InMemoryStore, MemoryEntry, MemoryStore, SemanticSearch};

fn bench_memory_store(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryStore::new();

    c.bench_function("memory_store", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = store
                    .store(black_box(MemoryEntry::new("test content", "fact")))
                    .await;
            })
        })
    });
}

fn bench_memory_text_search(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryStore::new();

    // Pre-populate
    rt.block_on(async {
        for i in 0..1000 {
            store
                .store(MemoryEntry::new(format!("memory entry {i}"), "fact"))
                .await
                .unwrap();
        }
    });

    c.bench_function("memory_text_search_1k", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = store.search_text(black_box("entry 500"), 10).await;
            })
        })
    });
}

fn bench_memory_semantic_search(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = InMemoryStore::new();

    // Pre-populate with embeddings
    rt.block_on(async {
        for i in 0..100 {
            let emb: Vec<f32> = (0..128).map(|j| ((i * 128 + j) as f32) / 16384.0).collect();
            store
                .store(MemoryEntry::new(format!("memory {i}"), "fact").with_embedding(emb))
                .await
                .unwrap();
        }
    });

    let query: Vec<f32> = (0..128).map(|i| (i as f32) / 128.0).collect();

    c.bench_function("semantic_search_100x128d", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = SemanticSearch::search(black_box(&store), black_box(&query), 10, 0.0).await;
            })
        })
    });
}

fn bench_cosine_similarity(c: &mut Criterion) {
    let a: Vec<f32> = (0..768).map(|i| (i as f32) / 768.0).collect();
    let b: Vec<f32> = (0..768).map(|i| ((768 - i) as f32) / 768.0).collect();

    c.bench_function("cosine_similarity_768d", |bench| {
        bench.iter(|| dsh_core_memory::cosine_similarity(black_box(&a), black_box(&b)))
    });
}

criterion_group!(
    memory_benches,
    bench_memory_store,
    bench_memory_text_search,
    bench_memory_semantic_search,
    bench_cosine_similarity,
);
criterion_main!(memory_benches);
