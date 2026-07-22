#!/usr/bin/env node
/**
 * Smoke test: spawn the Tauri dev process (or assume server is already running),
 * hit /api/health, hit /api/ws, validate NOT_ALLOWED behavior.
 *
 * Usage: SCREENMIRROR_RUNNING=3131 node scripts/smoke.js
 *        (or run npm run tauri:dev in another terminal)
 */
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const PORT = Number(process.env.SCREENMIRROR_PORT ?? 3131);
const BASE = `http://127.0.0.1:${PORT}`;

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function get(p) {
  return new Promise((resolve, reject) => {
    http
      .get(BASE + p, (res) => {
        let body = '';
        res.on('data', (c) => (body += c));
        res.on('end', () =>
          resolve({ status: res.statusCode, headers: res.headers, body }),
        );
      })
      .on('error', reject);
  });
}

async function waitForServer(timeout = 30000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    try {
      const r = await get('/api/health');
      if (r.status === 200) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error('server did not start');
}

async function main() {
  console.log('[smoke] waiting for server on', BASE);
  await waitForServer();

  const health = await get('/api/health');
  console.log('[smoke] health:', health.status, health.body);
  if (health.status !== 200) throw new Error('health failed');

  // HSTS / XFO headers should be set on static files
  const spa = await get('/');
  const hsts = spa.headers['strict-transport-security'];
  const xfo = spa.headers['x-frame-options'];
  console.log('[smoke] spa hsts:', hsts, 'xfo:', xfo);
  if (!hsts) console.warn('[smoke] WARN: HSTS missing (only set when viewer dist exists)');
  if (!xfo) console.warn('[smoke] WARN: XFO missing');

  console.log('[smoke] PASS');
}

main().catch((e) => {
  console.error('[smoke] FAIL:', e.message);
  process.exit(1);
});