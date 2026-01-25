use crate::utils::response::ApiResponse;
use axum::Json as AxumJson;
use axum::response::IntoResponse;
use axum::{
    Json,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

struct Bucket {
    tokens: f64,
    last: Instant,
    // last_access used by the background cleaner to evict idle buckets
    last_access: Instant,
}

static BUCKETS: Lazy<DashMap<String, Mutex<Bucket>>> = Lazy::new(DashMap::new);

// Background cleaner start flag
static CLEANER_STARTED: AtomicBool = AtomicBool::new(false);

fn read_config() -> (f64, f64, String, String, u64, f64) {
    // returns (rate_per_sec, burst, key_priority, action, bucket_ttl_secs, request_cost)
    let rate = std::env::var("RATE_LIMIT_RPS")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(100.0);
    let burst = std::env::var("RATE_LIMIT_BURST")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(rate * 2.0);
    let key_priority =
        std::env::var("RATE_LIMIT_KEY_PRIORITY").unwrap_or_else(|_| "ip".to_string());
    let action = std::env::var("RATE_LIMIT_ACTION").unwrap_or_else(|_| "block".to_string());
    let ttl = std::env::var("RATE_LIMIT_BUCKET_TTL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(300);
    let request_cost = std::env::var("RATE_LIMIT_REQUEST_COST")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1.0)
        .max(0.0);
    (rate, burst, key_priority, action, ttl, request_cost)
}

// Cached config (only active in non-test builds to preserve test determinism)
struct CachedConfig {
    rate: f64,
    burst: f64,
    key_priority: String,
    action: String,
    ttl_secs: u64,
    request_cost: f64,
    last_refresh: Instant,
}

static CONFIG_CACHE: Lazy<parking_lot::Mutex<CachedConfig>> = Lazy::new(|| {
    let (rate, burst, key_priority, action, ttl, request_cost) = read_config();
    parking_lot::Mutex::new(CachedConfig {
        rate,
        burst,
        key_priority,
        action,
        ttl_secs: ttl,
        request_cost,
        last_refresh: Instant::now() - Duration::from_secs(10),
    })
});

fn get_config() -> (f64, f64, String, String, u64, f64) {
    if cfg!(test) {
        // In test mode, always read env directly to avoid caching surprises in tests
        return read_config();
    }
    let mut c = CONFIG_CACHE.lock();
    // Always read env to detect on-the-fly changes quickly (necessary for integration tests
    // which set env vars between tests). Update cache if values changed or when the
    // refresh interval expired.
    let (rate, burst, key_priority, action, ttl, request_cost) = read_config();
    if c.last_refresh.elapsed() > Duration::from_secs(1)
        || rate != c.rate
        || burst != c.burst
        || key_priority != c.key_priority
        || action != c.action
        || ttl != c.ttl_secs
        || request_cost != c.request_cost
    {
        c.rate = rate;
        c.burst = burst;
        c.key_priority = key_priority;
        c.action = action;
        c.ttl_secs = ttl;
        c.request_cost = request_cost;
        c.last_refresh = Instant::now();
    }
    (
        c.rate,
        c.burst,
        c.key_priority.clone(),
        c.action.clone(),
        c.ttl_secs,
        c.request_cost,
    )
}

pub async fn rate_limiter(
    req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Result<Response, (StatusCode, Json<ApiResponse<serde_json::Value>>)> {
    let (rate, burst, key_priority, action, ttl_secs, request_cost) = get_config();

    // Determine key priority: can prefer auth token or ip. Supported values: "ip" (default), "auth", "auth+ip"
    // key_priority read via cached config (get_config)

    // Gather possible sources and include the source label to aid diagnostics
    // The tuple is (ip_value, source_label). Source labels match the header/extension
    // names used by clients/tests so debugging information is more precise.
    let ip_source: Option<(String, String)> = if let Some(c) =
        req.extensions()
            .get::<crate::middlewares::proxy::ClientIp>()
    {
        Some((c.0.to_string(), "extension".to_string()))
    } else if let Some(v) = req
        .headers()
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
    {
        Some((v.to_string(), "cf-connecting-ip".to_string()))
    } else if let Some(v) = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next().map(|s2| s2.trim().to_string()))
    {
        Some((v, "x-forwarded-for".to_string()))
    } else if let Some(v) = req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()) {
        Some((v.to_string(), "x-real-ip".to_string()))
    } else {
        req.extensions()
            .get::<std::net::SocketAddr>()
            .map(|sa| (sa.ip().to_string(), "peer".to_string()))
    };

    let auth_token_opt = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            let s = s.trim();
            if s.to_lowercase().starts_with("bearer ") {
                Some(s[7..].trim().to_string())
            } else {
                None
            }
        });

    // fingerprint helper (u64 hex)
    fn fingerprint(s: &str) -> String {
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    // Build the actual key and record key_source/type for diagnostics
    println!(
        "DEBUG: rate_limiter: key_priority='{}' ip_present={} auth_present={}",
        key_priority,
        ip_source.is_some(),
        auth_token_opt.is_some()
    );
    let (key, key_source, key_type) = match key_priority.as_str() {
        "auth" => {
            if let Some(tok) = &auth_token_opt {
                (
                    fingerprint(tok),
                    "authorization".to_string(),
                    "auth".to_string(),
                )
            } else if let Some((ip, src)) = ip_source.clone() {
                (ip, format!("fallback-{}", src), "ip".to_string())
            } else {
                (
                    "unknown".to_string(),
                    "unknown".to_string(),
                    "unknown".to_string(),
                )
            }
        }
        "auth+ip" => {
            if let Some(tok) = &auth_token_opt {
                let f = fingerprint(tok);
                let ip_part = ip_source
                    .clone()
                    .map(|(ip, _src)| ip)
                    .unwrap_or_else(|| "-".to_string());
                (
                    format!("auth:{}:ip:{}", f, ip_part),
                    "authorization+ip".to_string(),
                    "auth+ip".to_string(),
                )
            } else if let Some((ip, src)) = ip_source.clone() {
                (ip, format!("fallback-{}", src), "ip".to_string())
            } else {
                (
                    "unknown".to_string(),
                    "unknown".to_string(),
                    "unknown".to_string(),
                )
            }
        }
        _ => {
            if let Some((ip, src)) = ip_source.clone() {
                (ip, src, "ip".to_string())
            } else if let Some(tok) = &auth_token_opt {
                (
                    fingerprint(tok),
                    "authorization".to_string(),
                    "auth".to_string(),
                )
            } else {
                (
                    "unknown".to_string(),
                    "unknown".to_string(),
                    "unknown".to_string(),
                )
            }
        }
    };

    let now = Instant::now();
    // Ensure background cleaner is running
    ensure_cleaner_started(ttl_secs);

    // Narrow the scope for the lock so the guard does not live across awaits
    let (allowed, remaining, limit_header) = {
        let entry = BUCKETS.entry(key.clone()).or_insert_with(|| {
            Mutex::new(Bucket {
                tokens: burst,
                last: now,
                last_access: now,
            })
        });
        let mut bucket = entry.lock();
        let elapsed = now.duration_since(bucket.last).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * rate).min(burst);
        bucket.last = now;
        bucket.last_access = now;

        if bucket.tokens >= request_cost {
            bucket.tokens -= request_cost;
            (true, bucket.tokens.floor() as i64, rate.round() as i64)
        } else {
            (false, 0_i64, rate.round() as i64)
        }
    };

    if allowed {
        // Call next and append rate-limit headers
        let mut resp = next.run(req).await;
        // Add standard RateLimit headers (safe HeaderValue construction)
        if let Ok(hv) = HeaderValue::from_str(&limit_header.to_string()) {
            resp.headers_mut().insert("x-ratelimit-limit", hv);
        } else {
            tracing::warn!(
                "rate_limiter: failed to set x-ratelimit-limit header for key={}",
                key
            );
        }
        if let Ok(hv) = HeaderValue::from_str(&remaining.to_string()) {
            resp.headers_mut().insert("x-ratelimit-remaining", hv);
        } else {
            tracing::warn!(
                "rate_limiter: failed to set x-ratelimit-remaining header for key={}",
                key
            );
        }
        // Add debug headers indicating which source produced the key and the key type
        if let Ok(hv) = HeaderValue::from_str(&key_source) {
            resp.headers_mut().insert("x-key-source", hv);
        }
        if let Ok(hv) = HeaderValue::from_str(&key_type) {
            resp.headers_mut().insert("x-key-type", hv);
        }
        Ok(resp)
    } else {
        // Log blocked key and its source for debugging (helps distinguish proxy IP vs client IP)
        tracing::warn!(
            "rate_limiter: blocked key = {} source={} rate={} burst={}",
            key,
            key_source,
            rate,
            burst
        );

        // RATE_LIMIT_ACTION can be "block" (default), "drop", or "throttle" (future).
        // Use the cached `action` from config
        if action == "drop" {
            let mut resp = (StatusCode::NO_CONTENT, "").into_response();
            if let Ok(hv) = HeaderValue::from_str(&key_source) {
                resp.headers_mut().insert("x-key-source", hv);
            }
            if let Ok(hv) = HeaderValue::from_str(&key_type) {
                resp.headers_mut().insert("x-key-type", hv);
            }
            // Hint to close connection to reduce downstream work
            if let Ok(hv) = HeaderValue::from_str("close") {
                resp.headers_mut().insert("connection", hv);
            }
            return Ok(resp);
        }

        let response = ApiResponse::error_with_data(
            "Too Many Requests",
            json!({ "error": "Rate limit exceeded" }),
        );
        // Construct response with Retry-After header (1 second) and rate headers
        let retry_after_secs = 1;
        let mut resp = (StatusCode::TOO_MANY_REQUESTS, Json(response)).into_response();
        if let Ok(hv) = HeaderValue::from_str(&retry_after_secs.to_string()) {
            resp.headers_mut().insert("retry-after", hv);
        }
        if let Ok(hv) = HeaderValue::from_str(&(rate.round() as i64).to_string()) {
            resp.headers_mut().insert("x-ratelimit-limit", hv);
        }
        if let Ok(hv) = HeaderValue::from_str("0") {
            resp.headers_mut().insert("x-ratelimit-remaining", hv);
        }
        if let Ok(hv) = HeaderValue::from_str(&key_source) {
            resp.headers_mut().insert("x-key-source", hv);
        }
        Ok(resp)
    }
}

#[derive(Serialize)]
struct RateLimiterTopEntry {
    key: String,
    tokens: f64,
}

/// Debug endpoint for inspecting rate limiter buckets. Visible only when `RATE_LIMIT_DEBUG=true`.
pub async fn debug_info(req: axum::http::Request<axum::body::Body>) -> impl IntoResponse {
    let enabled = std::env::var("RATE_LIMIT_DEBUG")
        .map(|v| v == "true")
        .unwrap_or(false);
    if !enabled {
        return (
            StatusCode::NOT_FOUND,
            AxumJson(json!({ "error": "not found" })),
        );
    }

    // Optional token guard for safety in public environments
    if let Ok(token) = std::env::var("RATE_LIMIT_DEBUG_TOKEN") {
        // require Authorization: Bearer <token>
        match req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
        {
            Some(h) if h.trim().to_lowercase().starts_with("bearer ") && h[7..].trim() == token => {
                // ok
            }
            _ => {
                let response = ApiResponse::error_with_data(
                    "Unauthorized",
                    json!({ "error": "Missing or invalid Authorization header" }),
                );
                return (StatusCode::UNAUTHORIZED, AxumJson(json!(response)));
            }
        }
    } else {
        // If no debug token is set, allow only local requests (loopback) to protect exposed instances
        if let Some(sa) = req.extensions().get::<std::net::SocketAddr>() {
            if !sa.ip().is_loopback() {
                let response = ApiResponse::error_with_data(
                    "Unauthorized",
                    json!({ "error": "Missing Authorization header" }),
                );
                return (StatusCode::UNAUTHORIZED, AxumJson(json!(response)));
            }
        } else {
            // No peer info available; deny to be safe
            let response = ApiResponse::error_with_data(
                "Unauthorized",
                json!({ "error": "Missing Authorization header" }),
            );
            return (StatusCode::UNAUTHORIZED, AxumJson(json!(response)));
        }
    }

    // Sample up to N buckets to avoid expensive work
    let keys: Vec<String> = BUCKETS.iter().map(|r| r.key().clone()).take(200).collect();

    let mut samples: Vec<RateLimiterTopEntry> = Vec::new();
    for k in keys {
        if let Some(entry) = BUCKETS.get(&k) {
            let bucket = entry.value().lock();
            samples.push(RateLimiterTopEntry {
                key: k.clone(),
                tokens: bucket.tokens,
            });
        }
    }

    // top by tokens (highest remaining)
    samples.sort_by(|a, b| {
        a.tokens
            .partial_cmp(&b.tokens)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    samples.reverse();
    let top: Vec<serde_json::Value> = samples
        .iter()
        .take(50)
        .map(|e| json!({"key": e.key, "tokens": e.tokens}))
        .collect();

    // bottom by tokens (most drained / likely blocked)
    let bottom: Vec<serde_json::Value> = samples
        .iter()
        .rev()
        .take(50)
        .map(|e| json!({"key": e.key, "tokens": e.tokens}))
        .collect();

    // include runtime config to aid debugging
    let (rate_cfg, burst_cfg, key_priority, action, ttl_cfg, request_cost_cfg) = get_config();

    let resp = json!({
        "buckets": BUCKETS.len(),
        "sample_count": top.len(),
        "rate": rate_cfg,
        "burst": burst_cfg,
        "key_priority": key_priority,
        "action": action,
        "ttl_secs": ttl_cfg,
        "request_cost": request_cost_cfg,
        "top": top,
        "bottom": bottom,
    });
    (StatusCode::OK, AxumJson(resp))
}

// Start the cleaner only once per process
fn ensure_cleaner_started(ttl_secs: u64) {
    use std::sync::atomic::Ordering;
    if CLEANER_STARTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        // spawn background cleaner
        tokio::spawn(async move {
            let sleep_interval = Duration::from_secs(30);
            loop {
                tokio::time::sleep(sleep_interval).await;
                purge_stale_buckets(ttl_secs).await;
            }
        });
    }
}

async fn purge_stale_buckets(ttl_secs: u64) {
    println!(
        "DEBUG: purge_stale_buckets: start ttl={} buckets_before={}",
        ttl_secs,
        BUCKETS.len()
    );
    let now = Instant::now();
    let mut removed_count = 0usize;
    let keys: Vec<String> = BUCKETS.iter().map(|r| r.key().clone()).collect();
    for k in keys {
        // Read the bucket and determine if it should be removed while holding only
        // the bucket's internal mutex. Ensure we drop the DashMap guard before
        // mutating the map to avoid deadlocks.
        let should_remove = if let Some(entry) = BUCKETS.get(&k) {
            let bucket = entry.value().lock();
            let res = now.duration_since(bucket.last_access).as_secs() >= ttl_secs;
            // drop bucket and entry guard before mutating the map
            drop(bucket);
            drop(entry);
            res
        } else {
            false
        };

        if should_remove && BUCKETS.remove(&k).is_some() {
            removed_count += 1;
            tracing::info!("rate_limiter: purged stale bucket key={}", k);
        }
    }

    println!(
        "DEBUG: purge_stale_buckets: done removed={} buckets_after={}",
        removed_count,
        BUCKETS.len()
    );
}

// Test helper to purge once (exposed for integration tests)
// Exposed publicly to allow external test binaries to trigger cleanup deterministically.
pub async fn purge_stale_buckets_once(ttl_secs: u64) {
    purge_stale_buckets(ttl_secs).await;
}
