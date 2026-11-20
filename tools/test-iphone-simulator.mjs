import { spawn } from 'node:child_process';
import path from 'node:path';
import http from 'node:http';
import fs from 'node:fs';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const TAURI_BIN = path.join(ROOT, 'src-tauri', 'target', 'aarch64-apple-darwin', 'debug', 'screenmirror');
const VIEWER_DIST = path.join(ROOT, 'viewer', 'dist');
const OUT_DIR = path.join(__dirname, 'output');
const IPHONE_ID = '2C802D53-91DC-490A-BA32-BDFB8065E7FC';
const ROOM_ID = 'sim-iphone-room';
const PORT = 3136;
const BASE = `http://127.0.0.1:${PORT}`;

function get(p) {
  return new Promise((resolve, reject) => {
    http.get(BASE + p, (res) => {
      let body = '';
      res.on('data', (c) => (body += c));
      res.on('end', () => resolve({ status: res.statusCode, body }));
    }).on('error', reject);
  });
}

async function waitForServer(timeout = 30000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    try {
      const r = await get('/api/health');
      if (r.status === 200) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 300));
  }
  throw new Error('server did not start on port ' + PORT);
}

async function main() {
  fs.mkdirSync(OUT_DIR, { recursive: true });
  console.log('[iphone-sim] Starting ScreenMirror server on port', PORT);

  const proc = spawn(TAURI_BIN, ['--port', String(PORT)], {
    env: {
      ...process.env,
      RUST_LOG: 'info',
      SCREENMIRROR_PORT: String(PORT),
      SCREENMIRROR_TEST_ROOM: ROOM_ID,
      SCREENMIRROR_CAPTURE: 'test',
      SCREENMIRROR_MAX_DIM: '480',
      VIEWER_DIST,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  proc.stdout.on('data', (d) => process.stdout.write('[host] ' + d.toString()));
  proc.stderr.on('data', (d) => process.stderr.write('[host!] ' + d.toString()));

  const cleanup = () => {
    try { proc.kill('SIGKILL'); } catch {}
  };
  process.on('exit', cleanup);
  process.on('SIGINT', () => { cleanup(); process.exit(130); });
  process.on('SIGTERM', () => { cleanup(); process.exit(143); });

  await waitForServer();
  console.log('[iphone-sim] Host ready on port', PORT);

  const viewerUrl = `${BASE}/${ROOM_ID}`;
  console.log('[iphone-sim] Opening URL on iPhone Simulator:', viewerUrl);
  execSync(`xcrun simctl openurl ${IPHONE_ID} "${viewerUrl}"`);

  console.log('[iphone-sim] Waiting 10s for stable streaming...');
  await new Promise((r) => setTimeout(r, 10000));

  const shotPath = path.join(OUT_DIR, 'iphone-sim-live-stream.png');
  console.log('[iphone-sim] Taking screenshot of iPhone Simulator...');
  execSync(`xcrun simctl io ${IPHONE_ID} screenshot "${shotPath}"`);
  console.log('[iphone-sim] Screenshot saved to', shotPath);

  // Copy to artifacts dir
  const artifactPath = '/Users/blxie/.gemini/antigravity/brain/3a053a91-8a87-4e0c-9025-ade9b4c1d2c3/iphone-sim-live-stream.png';
  fs.copyFileSync(shotPath, artifactPath);
  console.log('[iphone-sim] Copied to artifact:', artifactPath);

  // Let it stream another 5s
  await new Promise((r) => setTimeout(r, 5000));
  cleanup();
}

main().catch((e) => {
  console.error('[iphone-sim] Error:', e);
  process.exit(1);
});
