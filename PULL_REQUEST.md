# chore: migrations + test cleanup

## Summary
- Add `create_users_table` migration
- Make index migration idempotent (safeguards against duplicate index)
- Ensure integration tests recreate clean DBs before running migrations (DROP + CREATE)
- Handle partial `_sqlx_migrations` entries at runtime with a one-time recovery attempt
- Fix HTTP verb in tests and remove unused imports

## Why
These changes make the migration flow more robust (idempotent and recoverable) and ensure tests run reliably on CI by starting from a clean database state.

## Changes
- migrations/20260122100826_create_users_table.sql: add CREATE TABLE IF NOT EXISTS users
- migrations/20260123121000_add_unique_index_on_users_email.sql: make index creation idempotent
- src/config/database.rs: attempt to detect and recover `VersionMismatch` by removing partial migration rows and retrying once
- tests/*: drop and recreate test DBs before running migrations; fix tests to match route verbs and remove unused imports
- removed duplicate `src/config.rs` and consolidated module in `src/config/mod.rs`

## Testing
All tests pass locally:
- Run: `cargo test` ✅
- Integration tests include DB setup that recreates test databases so they are isolated and repeatable

## Checklist
- [x] Code builds (`cargo build`) 
- [x] Tests pass (`cargo test`)
- [x] Migration changes are idempotent where necessary
- [x] PR description summarises intent and impact

## Notes for reviewers
- Review migration SQL for compatibility with your MySQL setup.
- The runtime recovery for partial migrations is a pragmatic safety net — it deletes the partial row and retries once; please review if you prefer a manual/explicit recovery approach instead.

---

All tests passed locally. Create PR with this branch: `chore/migrations-tests-cleanup`.
