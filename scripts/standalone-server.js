#!/usr/bin/env node
/**
 * Standalone server smoke test: builds the Tauri Rust lib and runs the signaling
 * router directly (without Electron GUI). This proves the network layer is healthy.
 *
 * For full E2E run `npm run tauri:dev` in a GUI session.
 */
import { spawn } from 'node:child_process';
import path from 'node:path';
import http from 'node:http';
import { fileURLToPath } from 'node:url';
import fs from 'node:fs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const RUST_DIR = path.join(ROOT, 'src-tauri');
const VIEWER_DIST = path.join(ROOT, 'viewer', 'dist');

async function get(p) {
  return new Promise((resolve, reject) => {
    http
      .get('http://127.0.0.1:3131' + p, (res) => {
        let body = '';
        res.on('data', (c) => (body += c));
        res.on('end', () =>
          resolve({ status: res.statusCode, headers: res.headers, body }),
        );
      })
      .on('error', reject);
  });
}

async function waitForServer(timeout = 15000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    try {
      const r = await get('/api/health');
      if (r.status === 200) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 300));
  }
  throw new Error('server did not start');
}

const serverBin = path.join(
  RUST_DIR,
  'target',
  'debug',
  'screenmirror-server',
);

if (!fs.existsSync(serverBin)) {
  console.log('[smoke] building standalone server...');
  await new Promise((resolve, reject) => {
    const proc = spawn('cargo', ['build', '--bin', 'screenmirror-server'], {
      cwd: RUST_DIR,
      stdio: 'inherit',
    });
    proc.on('exit', (code) =>
      code === 0 ? resolve() : reject(new Error(`build exit ${code}`)),
    );
  });
}

console.log('[smoke] starting standalone server on 3131...');
const proc = spawn(serverBin, [], {
  env: { ...process.env, RUST_LOG: 'info', VIEWER_DIST },
  stdio: ['ignore', 'pipe', 'pipe'],
});
proc.stdout.on('data', (d) => process.stdout.write('[srv] ' + d.toString()));
proc.stderr.on('data', (d) => process.stderr.write('[srv!] ' + d.toString()));

const cleanup = () => {
  try { proc.kill('SIGKILL'); } catch {}
};
process.on('exit', cleanup);
process.on('SIGINT', () => { cleanup(); process.exit(130); });

try {
  await waitForServer();
  console.log('[smoke] health endpoint OK');

  const spa = await get('/somefakeRoomId');
  console.log('[smoke] SPA fallback status:', spa.status, 'len:', spa.body.length);

  console.log('[smoke] PASS');
  cleanup();
  process.exit(0);
} catch (e) {
  console.error('[smoke] FAIL:', e.message);
  cleanup();
  process.exit(1);
}