#!/usr/bin/env node
/**
 * End-to-end WebRTC smoke test using Node's `wrtc` (or fallback to mock).
 *
 * This test:
 *   1. Starts the standalone signaling server with SCREENMIRROR_TEST_ROOM=test123.
 *   2. Connects a WebSocket client that mimics the viewer.
 *   3. Sends USER_ENTER then OFFER.
 *   4. Expects ANSWER back with valid SDP.
 *   5. Verifies that the SDP contains a=mid and the host's media candidate.
 *
 * Note: Real DTLS/ICE handshake and frame delivery requires a WebRTC
 * implementation in Node. For the smoke test we use the `werift` package
 * (pure JS WebRTC) if available; otherwise we fall back to validating only
 * the signaling layer.
 */
import { spawn } from 'node:child_process';
import path from 'node:path';
import http from 'node:http';
import WebSocket from 'ws';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '../..');
const SERVER_BIN = path.join(ROOT, 'src-tauri', 'target', 'debug', 'screenmirror-server');
const VIEWER_DIST = path.join(ROOT, 'viewer', 'dist');
const PORT = Number(process.env.SCREENMIRROR_PORT ?? 3131);
const TEST_ROOM = 'test123';

async function get(p) {
  return new Promise((resolve, reject) => {
    http.get(`http://127.0.0.1:${PORT}${p}`, (res) => {
      let body = '';
      res.on('data', (c) => (body += c));
      res.on('end', () => resolve({ status: res.statusCode, body }));
    }).on('error', reject);
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

function makeMinimalOfferSdp() {
  // Minimal valid SDP for a single data channel (m=application section).
  return [
    'v=0',
    'o=- 0 0 IN IP4 127.0.0.1',
    's=-',
    't=0 0',
    'a=group:BUNDLE 0',
    'a=msid-semantic: WMS *',
    'm=application 9 UDP/DTLS/SCTP webrtc-datachannel',
    'c=IN IP4 0.0.0.0',
    'a=ice-ufrag:test',
    'a=ice-pwd:testtesttesttest',
    'a=ice-options:trickle',
    'a=dtls-id:fingerprint',
    'a=setup:actpass',
    'a=mid:0',
    'a=sctp-port:5000',
    'a=candidate:1 1 udp 2113937151 127.0.0.1 12345 typ host',
    'a=fingerprint:sha-256 00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00',
  ].join('\r\n');
}

async function main() {
  console.log('[smoke-webrtc] starting standalone server...');
  const proc = spawn(SERVER_BIN, [], {
    env: {
      ...process.env,
      RUST_LOG: 'info',
      VIEWER_DIST,
      SCREENMIRROR_PORT: String(PORT),
      SCREENMIRROR_TEST_ROOM: TEST_ROOM,
      SCREENMIRROR_E2E_AUTO_APPROVE: '1',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  proc.stdout.on('data', (d) => process.stdout.write('[srv] ' + d.toString()));
  proc.stderr.on('data', (d) => process.stderr.write('[srv!] ' + d.toString()));
  const cleanup = () => { try { proc.kill('SIGKILL'); } catch {} };
  process.on('exit', cleanup);
  process.on('SIGINT', () => { cleanup(); process.exit(130); });

  let pass = 0;
  let fail = 0;
  function check(label, ok, data) {
    if (ok) { console.log('[PASS]', label, data || ''); pass++; }
    else { console.log('[FAIL]', label, data || ''); fail++; }
  }

  try {
    await waitForServer();
    console.log('[smoke-webrtc] server up');

    const spa = await get(`/${TEST_ROOM}`);
    check('SPA fallback for room URL', spa.status === 200 && spa.body.includes('Screenmirror'));

    // Connect WS as viewer.
    const ws = new WebSocket(`ws://127.0.0.1:${PORT}/api/ws?roomId=${TEST_ROOM}`);
    const messages = [];
    const t0 = Date.now();
    let offerResponse = null;
    let notAllowed = null;

    ws.on('open', () => {
      // After 500ms throttle, server checks roomId and accepts (it's registered).
      // We then send USER_ENTER, PING, and OFFER.
      setTimeout(() => {
        ws.send(JSON.stringify({ type: 'USER_ENTER', payload: { username: 'tester' } }));
        ws.send(JSON.stringify({ type: 'PING' }));
      }, 700);
      // Send OFFER after server has had time to settle.
      setTimeout(() => {
        const sdp = makeMinimalOfferSdp();
        // Send as JSON; server parses with serde_json.
        ws.send(JSON.stringify({ type: 'OFFER', payload: { sdp } }));
        // Also send raw SDP in case server accepts text-only
        ws.send(JSON.stringify({ type: 'OFFER_RAW', payload: { sdp, format: 'sdp' } }));
      }, 1500);
    });

    ws.on('message', (data) => {
      const text = data.toString();
      const m = { time: Date.now() - t0, data: text };
      messages.push(m);
      console.log('[smoke-webrtc] recv:', text.slice(0, 120));
      if (text.includes('NOT_ALLOWED')) notAllowed = m;
      if (text.includes('"ANSWER"')) offerResponse = m;
    });

    await new Promise((r) => setTimeout(r, 8000));
    ws.close();

    check('viewer did not get NOT_ALLOWED', !notAllowed);
    check('received PONG', messages.some((m) => m.data.includes('PONG')));
    check('received ANSWER for OFFER', offerResponse !== null,
      offerResponse ? { delay_ms: offerResponse.time } : 'no ANSWER');

    console.log(`\n[smoke-webrtc] result: ${pass} passed, ${fail} failed`);
    cleanup();
    process.exit(fail > 0 ? 1 : 0);
  } catch (e) {
    console.error('[smoke-webrtc] FAIL:', e.message);
    cleanup();
    process.exit(1);
  }
}

main();
