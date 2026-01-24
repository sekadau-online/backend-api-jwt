K6 load testing scripts for backend-api-jwt

Prerequisites
- k6 installed: https://k6.io/docs/getting-started/installation/
- Your server running locally (default: http://127.0.0.1:3002) and migrations applied.
- Ensure `JWT_SECRET` is set in the environment where the server runs (k6 doesn't set server env).

Quick run examples

1) Basic register+login flow (short load test):

   BASE_URL=http://127.0.0.1:3002 k6 run tests/k6/login.js

   You can override VUs / duration:

   VUS=50 DURATION=30s BASE_URL=http://127.0.0.1:3002 k6 run tests/k6/login.js

2) Read-heavy users test using a single shared test user (or provide TEST_EMAIL/TEST_PASSWORD):

   TEST_EMAIL=admin@example.test TEST_PASSWORD=pass BASE_URL=http://127.0.0.1:3002 k6 run tests/k6/users.js

   Or let the script create a shared user for you (not recommended for strict reproducibility):

   BASE_URL=http://127.0.0.1:3002 k6 run tests/k6/users.js

Recommendations
- Run these tests against a staging instance (not production) and ensure database can handle writes.
- Start with low VUs and short duration, then increase gradually.
- Monitor DB (connection pool), CPU, memory, and response times during tests.

Interpreting results
- Look at p(95) and error rates in the k6 output.
- If you see 5xx errors, inspect the server logs and DB connection limits.

CI integration
- You can add a GitHub Action that runs k6 container and fails on thresholds. Keep tests short for PRs and longer for nightly runs.
