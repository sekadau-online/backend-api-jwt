# backend-api-jwt

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
- **CORS_ALLOWED_ORIGINS** — optional comma-separated list of allowed origins (e.g. `https://example.com,https://app.example.com`). When set, CORS is enabled. In the current implementation this still falls back to a permissive policy; we can tighten it to strict matching if you prefer.

Tip: CORS is configured inside `create_app(..)` so tests and other programmatic runners will share the same behavior as the main server.

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

3. The server listens on `http://<APP_HOST>:<APP_PORT>` by default (defaults: `APP_HOST=127.0.0.1`, `APP_PORT=3002`). You can configure the bind host via the `APP_HOST` environment variable (set it to `0.0.0.0` to listen on all interfaces).

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
Add integration tests to validate handler behavior (suggestion: use a test DB or mock). Running `cargo test` will run available unit tests.

### Using `.env.test` (recommended)
A sample test environment file `.env.test` is provided. It contains example values for running integration tests locally. **Do not** commit real credentials to this file — replace placeholders with your local test credentials.

Example usage:

```bash
# copy the template to .env so dotenvy picks it up
cp .env.test .env
# create the test database (adjust credentials as needed)
mysql -u root -p -e "CREATE DATABASE IF NOT EXISTS db_backend_api_jwt_test;"
# run integration tests
cargo test --test integration_register
```

If `DATABASE_URL` is not set, integration tests will be skipped with a helpful message. This prevents accidental test runs without proper test DB setup.

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