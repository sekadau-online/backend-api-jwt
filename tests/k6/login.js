import http from 'k6/http';
import { check, sleep } from 'k6';

export let options = {
  vus: __ENV.VUS ? Number(__ENV.VUS) : 20,
  duration: __ENV.DURATION ? __ENV.DURATION : '30s',
  thresholds: {
    http_req_duration: ['p(95)<500'], // 95% requests should be < 500ms
    'http_req_failed{status:500}': ['rate==0'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://127.0.0.1:3002';

function randomEmail() {
  return `loaduser_${__VU}_${Math.floor(Math.random() * 1e6)}@example.test`;
}

export default function () {
  // 1) Register
  const email = randomEmail();
  const password = 'password123';
  const registerRes = http.post(`${BASE_URL}/register`, JSON.stringify({ name: 'Load User', email, password }), {
    headers: { 'Content-Type': 'application/json' },
  });

  check(registerRes, {
    'register: status 201 or 409': (r) => r.status === 201 || r.status === 409,
  });

  // 2) Login
  const loginRes = http.post(`${BASE_URL}/login`, JSON.stringify({ email, password }), {
    headers: { 'Content-Type': 'application/json' },
  });

  check(loginRes, {
    'login: status 200': (r) => r.status === 200,
    'login: token exists': (r) => {
      try {
        const json = r.json();
        return json && json.data && json.data.token;
      } catch (e) {
        return false;
      }
    },
  });

  // 3) Use token to call /users (protected)
  if (loginRes.status === 200) {
    const token = loginRes.json().data.token;
    const usersRes = http.get(`${BASE_URL}/users`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    check(usersRes, {
      'users: status 200': (r) => r.status === 200,
    });
  }

  sleep(1);
}
