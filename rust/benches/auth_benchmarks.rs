//! Auth benchmarks — API key verification, rate limiting, quota checks.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use dsh_seam_auth::{AuthManager, QuotaConfig, QuotaManager, RateLimitConfig, RateLimiter};

fn bench_auth_verify(c: &mut Criterion) {
    let auth = AuthManager::new();
    auth.register_key("sk-bench", "bench key", "user1", vec!["chat".into()]);

    c.bench_function("auth_verify", |b| {
        b.iter(|| {
            let _ = auth.verify(black_box("sk-bench"));
        })
    });
}

fn bench_auth_permission_check(c: &mut Criterion) {
    let auth = AuthManager::new();
    auth.register_key(
        "sk-bench",
        "bench key",
        "user1",
        vec!["chat".into(), "admin".into()],
    );

    c.bench_function("auth_permission_check", |b| {
        b.iter(|| {
            let _ = auth.check_permission(black_box("sk-bench"), black_box("admin"));
        })
    });
}

fn bench_rate_limit_check(c: &mut Criterion) {
    let limiter = RateLimiter::new(RateLimitConfig {
        max_tokens: 1000,
        refill_rate: 100.0,
    });

    c.bench_function("rate_limit_check", |b| {
        b.iter(|| {
            let _ = limiter.check(black_box("user1"));
        })
    });
}

fn bench_quota_check(c: &mut Criterion) {
    let quota = QuotaManager::new(QuotaConfig {
        daily_requests: 10000,
        daily_tokens: 10_000_000,
        monthly_requests: 300_000,
        monthly_tokens: 300_000_000,
    });

    c.bench_function("quota_check", |b| {
        b.iter(|| {
            let _ = quota.check(black_box("user1"));
        })
    });
}

fn bench_quota_record(c: &mut Criterion) {
    let quota = QuotaManager::new(QuotaConfig::default());

    c.bench_function("quota_record", |b| {
        b.iter(|| {
            quota.record(black_box("user1"), black_box(100));
        })
    });
}

criterion_group!(
    auth_benches,
    bench_auth_verify,
    bench_auth_permission_check,
    bench_rate_limit_check,
    bench_quota_check,
    bench_quota_record,
);
criterion_main!(auth_benches);
