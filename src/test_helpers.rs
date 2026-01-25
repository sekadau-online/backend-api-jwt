/// Make the rate limiter permissive for tests and purge any existing buckets.
/// Intended to be called at the start of integration tests which may be affected by global in-memory buckets.
pub async fn make_rate_limiter_permissive_and_purge() {
    unsafe {
        std::env::set_var("RATE_LIMIT_RPS", "10000");
    }
    unsafe {
        std::env::set_var("RATE_LIMIT_BURST", "20000");
    }
    unsafe {
        std::env::set_var("RATE_LIMIT_REQUEST_COST", "0.2");
    }
    // Purge stale buckets synchronously via the test helper in the rate limiter module
    crate::middlewares::rate_limiter::purge_stale_buckets_once(0).await;
}
