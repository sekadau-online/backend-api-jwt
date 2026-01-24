import http from 'k6/http';
import { check, sleep } from 'k6';

export let options = {
  vus: __ENV.VUS ? Number(__ENV.VUS) : 50,
  duration: __ENV.DURATION ? __ENV.DURATION : '1m',
  thresholds: {
    http_req_duration: ['p(95)<600'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://127.0.0.1:3002';
const TEST_EMAIL = __ENV.TEST_EMAIL;
const TEST_PASSWORD = __ENV.TEST_PASSWORD;

let token = null;

export function setup() {
  if (!TEST_EMAIL || !TEST_PASSWORD) {
    // Try to register a shared test user if credentials aren't provided
    const email = `k6_shared_${Date.now()}@example.test`;
    const password = 'password123';
    const r = http.post(`${BASE_URL}/register`, JSON.stringify({ name: 'K6 Shared', email, password }), {
      headers: { 'Content-Type': 'application/json' },
    });
    const login = http.post(`${BASE_URL}/login`, JSON.stringify({ email, password }), {
      headers: { 'Content-Type': 'application/json' },
    });
    if (login.status !== 200) {
      throw new Error('Failed to create/login shared k6 user');
    }
    return login.json().data.token;
  }

  const login = http.post(`${BASE_URL}/login`, JSON.stringify({ email: TEST_EMAIL, password: TEST_PASSWORD }), {
    headers: { 'Content-Type': 'application/json' },
  });
  if (login.status !== 200) {
    throw new Error('Failed to login test user');
  }
  return login.json().data.token;
}

export default function (tokenFromSetup) {
  token = tokenFromSetup;

  // GET users (read-intensive path)
  const r = http.get(`${BASE_URL}/users`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  check(r, { 'get users 200': (res) => res.status === 200 });

  // occasionally create a user (low rate)
  if (Math.random() < 0.05) {
    const email = `k6_user_${__VU}_${Math.floor(Math.random() * 1e6)}@example.test`;
    const create = http.post(`${BASE_URL}/users`, JSON.stringify({ name: 'k6 user', email, password: 'password123' }), {
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    });
    check(create, { 'create user 201 or 409': (res) => res.status === 201 || res.status === 409 });
  }

  sleep(1);
}
