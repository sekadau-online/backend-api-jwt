# backend-api-jwt_v1

**Rust backend API** with JWT auth helpers, registration handler, input validation, and automatic database migrations. 🚀

---

## ⚙️ Overview
A small Rust REST API built with Axum, SQLx (MySQL), Serde, and JSON Web Tokens. It includes a user registration flow with validation, password hashing (bcrypt), and structured API responses.

## ✅ Features
- Registration endpoint with validation (name, email, password)
- Password hashing with bcrypt
- Duplicate email detection (returns 409 Conflict)
- Structured JSON responses for success and errors
- Automatic SQL migration on startup (via `sqlx::migrate!`)
- Tracing initialized for better logs

---

## 🔧 Requirements
- Rust (latest stable)
- MySQL server
- `cargo` and `mysql` CLI for manual migrations (optional)

## Environment
Create a `.env` file in project root (do not commit it). Required variables:

```env
APP_PORT=3000
DATABASE_URL=mysql://user:password@<DB_HOST>:3306/db_backend_api_jwt
# Alternatively set DB_HOST and compose DATABASE_URL in your shell:
# export DB_HOST=127.0.0.1 && export DATABASE_URL="mysql://user:password@$DB_HOST:3306/db_backend_api_jwt"
JWT_SECRET=<a long secret string>
```

> Tip: add `.env` to `.gitignore`.

---

## � CORS configuration 🔁

By default CORS is **disabled**. To enable it for development or testing you can use environment variables:

- **ENABLE_CORS=true** — enables permissive CORS (allows any origin). Good for local/dev testing.
- **CORS_ALLOWED_ORIGINS** — a comma-separated list of allowed origins (e.g. `https://app.example.com,https://admin.example.com`), or the special value `*` to allow any origin (permissive).

Examples:
- `ENABLE_CORS=true` (quick permissive enable)
- `CORS_ALLOWED_ORIGINS=*` (explicit wildcard — treated as permissive)
- `CORS_ALLOWED_ORIGINS=https://app.example.com,https://admin.example.com` (restrict to specific origins)

Tip: CORS is configured inside `create_app(..)` so tests and other programmatic runners will share the same behavior as the main server. For production, prefer restricting origins with `CORS_ALLOWED_ORIGINS`.

## 🏗️ Application structure

The app router and middleware are implemented so they can be reused in tests and the running server:

- `src/app.rs`
  - `build_router()` — build an `axum::Router` **without** a DB connection (useful for unit tests and route-level checks).
  - `create_app(pool)` — wrap `build_router()` and add `Extension(pool)` for runtime (used by `main.rs`).
- `src/lib.rs` re-exports `create_app` for convenience (`pub use app::create_app;`).

Why this is useful:
- **Testability** — call `build_router()` in unit tests to verify route registration and middleware without spinning a database.
- **Separation of concerns** — routing is separate from runtime wiring (DB, app-level extension).

Example unit test (smoke):

```rust
#[test]
fn build_router_smoke() {
    let _router = backend_api_jwt::app::build_router();
}
```

In `main.rs` use `create_app` as before:

```rust
let app = backend_api_jwt::create_app(db_pool.clone());
```

---

## �🏃 Running the project
1. Start MySQL and create the database:

```bash
mysql -u root -p -e "CREATE DATABASE IF NOT EXISTS db_backend_api_jwt;"
```

2. Build & run the app (it will apply migrations automatically):

```bash
cargo run
```
or run with watch 
```bash
cargo watch -q -c -w src/ -x run
```
3. The server listens on `http://<APP_HOST>:<APP_PORT>` by default (defaults: `APP_HOST=127.0.0.1`, `APP_PORT=3002`). You can configure the bind host via the `APP_HOST` environment variable (set it to `0.0.0.0` to listen on all interfaces).

### Quick run & verify ✅

Start the server in one terminal:

```bash
cargo run
```

You should see output similar to:

```
Compiling backend-api-jwt v0.1.0
Finished `dev` profile [...]
Running `target/debug/backend-api-jwt`
Listening on http://127.0.0.1:3000
```

Verify the process is listening on the configured port (example uses `ss`):

```bash
ss -ltnp | grep 3000
```

Quick CORS preflight test (only if CORS enabled):

```bash
curl -i -X OPTIONS http://127.0.0.1:3000/register \
  -H 'Origin: http://example.com' \
  -H 'Access-Control-Request-Method: POST'
```

A successful CORS response will contain an `Access-Control-Allow-Origin` header. For simple endpoint checks you can `curl` an existing route; note some routes are protected and require `Authorization`.

---

## 📚 Migrations
Migrations are stored in `migrations/` and are applied automatically on startup using `sqlx::migrate!`. To run or prepare migrations manually:

```bash
# (optional) prepare offline SQLX cache
cargo sqlx prepare -- --lib

# Apply manually via mysql
mysql -u root -p db_backend_api_jwt < migrations/20260122100826_create_users_table.sql
```

---

## 📬 API — Register
- **URL**: `POST /register`
- **Content-Type**: `application/json`

Request body (JSON):
```json
{
  "name": "Alice",
  "email": "alice@example.com",
  "password": "secret123"
}
```

Validation rules:
- `name`: non-empty
- `email`: valid email format
- `password`: minimum 6 characters

Success response (201):
```json
{
  "success": true,
  "message": "User registered",
  "data": { "id": 1, "name": "Alice", "email": "alice@example.com", "created_at": "..." }
}
```

Validation error (400):
```json
{
  "success": false,
  "message": "Validation error",
  "data": { "errors": { "email": ["Invalid email format"], "password": ["Password must be at least 6 characters long"] } }
}
```

Duplicate email (409):
```json
{
  "success": false,
  "message": "Conflict",
  "data": { "error": "Email already registered", "field": "email" }
}
```

Server/database error (500) returns structured `data` with `error` and `details` fields for debugging in development.

---

## 🔒 Security & Production Notes
- Do NOT commit `.env` or secrets to git. Use environment variables or secret managers in production.
- Consider hiding DB error details from responses in production; the app currently returns detailed `data.details` for debugging.
- Use strong `JWT_SECRET` and protect it.

---

## 🧪 Tests
Add integration tests to validate handler behavior (suggestion: use a test DB or mock). Running `cargo test` will run available unit tests and integration tests.

There is also a focused test file for router-level behavior without requiring a DB:

- `tests/app_router.rs` contains async oneshot tests that exercise CORS preflight behavior using `build_router()` (no DB required). These tests use `tower::util::ServiceExt::oneshot` to dispatch requests to the router.

To run the router-level tests only:

```bash
cargo test --test app_router
```

Developer note: `tower` is listed under `dev-dependencies` to enable `oneshot` helpers used in these tests.

### Using `.env.test` (recommended)
A sample test environment file `.env.test` is provided. It contains example values for running integration tests locally. **Do not** commit real credentials to this file — replace placeholders with your local test credentials.

Example usage:

```bash
# copy the template to .env so dotenvy picks it up
cp .env.test .env
# create the test database (adjust credentials as needed)
mysql -u root -p -e "CREATE DATABASE IF NOT EXISTS db_backend_api_jwt_test;"
# run integration tests
cargo test --tests
```

If `DATABASE_URL` is not set, integration tests will be skipped with a helpful message. This prevents accidental test runs without proper test DB setup.

---

## Rate limiting
This service includes an in-memory per-IP token-bucket rate limiter middleware to protect from request floods in single-instance deployments.

Configuration (via environment variables):
- `RATE_LIMIT_RPS` — allowed token refill rate per second per key (default: 100)
- `RATE_LIMIT_BURST` — maximum tokens the bucket can hold (default: `RATE_LIMIT_RPS * 2`)
- `RATE_LIMIT_REQUEST_COST` — cost (float) consumed per request (default: `1.0`). Use values < 1.0 to allow higher effective throughput per key (useful when many clients are aggregated into a single IP behind NAT or an edge). Note: this changes token consumption, not how shortages are reported (see `RATE_LIMIT_ACTION`).
- `RATE_LIMIT_ACTION` — action when a bucket is empty (supported: `drop` = close/204, `throttle` = 429). Default: `drop`.
- `RATE_LIMIT_BUCKET_TTL_SECS` — how long (seconds) a bucket is considered idle before the background cleaner evicts it (default 300).
- `RATE_LIMIT_DEBUG` — `true` to expose `/debug/rate_limiter` endpoint for inspection (use only in dev/test). Protect with `RATE_LIMIT_DEBUG_TOKEN`.
- `RATE_LIMIT_DEBUG_TOKEN` — bearer token used to authorize access to `/debug/rate_limiter` if `RATE_LIMIT_DEBUG=true`.

Cleanup / eviction
- The rate limiter runs a background cleaner (every 30 seconds) which evicts buckets that have not been accessed for `RATE_LIMIT_BUCKET_TTL_SECS` seconds. This keeps memory bounded and ensures stale client buckets are removed automatically.
- For tests and one-off maintenance you can trigger a single cleanup run programmatically by calling `backend_api_jwt::middlewares::rate_limiter::purge_stale_buckets_once(ttl_secs)` from a test or helper binary. Useful in integration tests to ensure deterministic state.
- The admin `POST /debug/rate_limiter` action can also be used to drop individual buckets by key (see examples above).

Notes:
- The implementation lives in `src/middlewares/rate_limiter.rs` and is intended for single-instance deployments. For multi-instance setups, use a centralized rate limiter (Redis, API Gateway) to coordinate limits across replicas.
- When running load tests, tune `RATE_LIMIT_RPS`, `RATE_LIMIT_BURST`, and `RATE_LIMIT_REQUEST_COST` so you test the backend behavior and not the rate limiter itself.

Quick local troubleshooting:
- Enable debug endpoint for testing:

```bash
export RATE_LIMIT_DEBUG=true
export RATE_LIMIT_DEBUG_TOKEN=<a-secret-token>
```

- Inspect current buckets (from a trusted host):

```bash
curl -H "Authorization: Bearer $RATE_LIMIT_DEBUG_TOKEN" http://127.0.0.1:8000/debug/rate_limiter | jq .
```

Example (increase limits for performance testing):

```bash
# set a higher per-IP rate for a controlled load test
export RATE_LIMIT_RPS=500
export RATE_LIMIT_BURST=1000
# reduce per-request cost to 0.2 (effectively increase capacity x5)
export RATE_LIMIT_REQUEST_COST=0.2
```

For aggressive testing you can temporarily reduce `RATE_LIMIT_REQUEST_COST` to make a key more permissive or lower `RATE_LIMIT_BURST`/`RATE_LIMIT_RPS` to more easily reproduce blocking behavior during a short test.


---

## Proxy / Cloudflare
When running behind proxies (e.g., Cloudflare), the app can resolve the originating client IP from the request headers. To avoid spoofing, configure a trusted proxy list:

- `TRUSTED_PROXIES` — comma-separated list of CIDRs to trust (e.g. Cloudflare IP ranges). Only when a request comes from a trusted proxy will the middleware honor `CF-Connecting-IP`, `X-Forwarded-For`, or `X-Real-IP` headers.

Example (trust Cloudflare ranges in production):
```bash
export TRUSTED_PROXIES="173.245.48.0/20,103.21.244.0/22,103.22.200.0/22"
```

Notes:
- Do **not** set `TRUSTED_PROXIES=0.0.0.0/0` in production — this trusts all upstreams and allows IP spoofing.
- The middleware populates an extension `ClientIp` so other middleware (rate limiter, auth) can use the resolved IP.

### Fetch Cloudflare IP ranges & verify setup 🔍
- Fetch Cloudflare's current IP lists (IPv4 and IPv6):
```bash
curl -fsS https://www.cloudflare.com/ips-v4 -o -
curl -fsS https://www.cloudflare.com/ips-v6 -o -
```
- Update `TRUSTED_PROXIES` quickly (example writes to `.env`):
```bash
# join ranges with commas and write/update .env
CF4=$(curl -fsS https://www.cloudflare.com/ips-v4 | paste -s -d, -)
CF6=$(curl -fsS https://www.cloudflare.com/ips-v6 | paste -s -d, -)
sed -i "/^TRUSTED_PROXIES=/c\TRUSTED_PROXIES=${CF4},${CF6}" .env || echo "TRUSTED_PROXIES=${CF4},${CF6}" >> .env
```
- Quick runtime check (local dev): send a request simulating Cloudflare header and inspect `x-key-source` header:
```bash
# send request with cf header and show response headers
curl -I -H "cf-connecting-ip: 203.0.113.55" http://127.0.0.1:${APP_PORT:-8000}/users
# expect: x-key-source: cf-connecting-ip (if TRUSTED_PROXIES contains the proxy that sent the request)
```
- Inspect rate-limiter buckets (requires `RATE_LIMIT_DEBUG=true` and `RATE_LIMIT_DEBUG_TOKEN`):
```bash
curl -H "Authorization: Bearer $RATE_LIMIT_DEBUG_TOKEN" http://127.0.0.1:${APP_PORT:-8000}/debug/rate_limiter | jq .
```

- Admin actions on the rate limiter (requires `RATE_LIMIT_DEBUG=true` and authorization):

  - Drop one or more buckets by key, or drop keys included in the `top`/`bottom` lists returned by the GET handler. Example (drop a single bottom key):

  ```bash
  curl -X POST -H "Authorization: Bearer $RATE_LIMIT_DEBUG_TOKEN" \
       -H "Content-Type: application/json" \
       -d '{ "action": "drop", "bottom": [{ "key": "<bucket_key_here>" }] }' \
       http://127.0.0.1:${APP_PORT:-8000}/debug/rate_limiter | jq .
  ```

  - You can also send a list of explicit keys to drop:

  ```bash
  curl -X POST -H "Authorization: Bearer $RATE_LIMIT_DEBUG_TOKEN" \
       -H "Content-Type: application/json" \
       -d '{ "action": "drop", "keys": ["key1","key2"] }' \
       http://127.0.0.1:${APP_PORT:-8000}/debug/rate_limiter | jq .
  ```

  - The POST returns a JSON object with the number of removed buckets and the current bucket count, for example: `{ "removed": 1, "buckets": 5 }`.

  Note: if `RATE_LIMIT_DEBUG_TOKEN` is not set, the debug endpoints are only accessible from loopback (127.0.0.1) to avoid exposing admin controls.

Notes:
- Include IPv6 ranges from Cloudflare if you accept IPv6 traffic (e.g. `2400:cb00::/32`, `2606:4700::/32`, ...).
- Update these ranges periodically — Cloudflare may add/change ranges.

---

## Operational notes
Some operational tips and recommended settings for running under load:

- DB connection pool size (`DB_POOL_SIZE`): set in `.env` (default 20). Tune to your MySQL `max_connections` and total number of app instances (e.g., `DB_POOL_SIZE * instances <= max_connections`).

- File descriptor limit (NOFILE): high concurrency and many sockets may hit OS limits ("Too many open files"). Increase limits if needed:
  - For quick testing: `ulimit -n 65536` before starting the process
  - For systemd services, set permanently via an override:
    ```ini
    [Service]
    LimitNOFILE=65536
    ```
    then `sudo systemctl daemon-reload && sudo systemctl restart your-service`

- Redis-backed rate limiting (recommended for multi-instance): consider a Redis-based limiter using an atomic Lua script for consistent global limits. See `src/middlewares` if you want to add a Redis POC.

- k6 load testing tips:
  - Use staged ramps and increase VUs gradually (e.g., 50→100→200) — avoid jumping to 1000 VUs on a single instance.
  - Use a shared test user for write-heavy flows, or reduce write rate to avoid DB contention during load tests.

These configuration defaults and checks help avoid common saturation issues (DB connection exhaustion, FD limits, spike-related timeouts).
---

## 🚀 CI / GitHub Actions (Integration tests)

We include a GitHub Actions workflow that runs integration tests against a MySQL service. To run the workflow, set these repository secrets:

- **MYSQL_ROOT_PASSWORD** (required) — root password for the MySQL service used by the workflow
- **JWT_SECRET** (required) — secret used to sign tokens during tests

The workflow also relies on `DATABASE_URL` being set inside the job (the workflow builds this from `MYSQL_ROOT_PASSWORD`). The job will fail fast if required secrets are missing.

Tip: Use `gh secret set NAME --body 'value' --repo OWNER/REPO` to add secrets via the GitHub CLI.

---

## Contributing
Contributions are welcome. Open an issue or PR and follow standard git workflow (fetch, rebase, push).

---

If you want, I can also add an endpoint to check email availability or add integration tests for registration — tell me which. ⚡