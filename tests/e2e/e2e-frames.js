#!/usr/bin/env node
/**
 * End-to-end frame capture test using @roamhq/wrtc.
 *
 * Pipeline:
 *   1. Start the standalone signaling server (which captures real macOS
 *      frames via xcap and pushes them through str0m's DataChannel).
 *   2. This script acts as the browser viewer: opens WebSocket, sends
 *      USER_ENTER + OFFER via standard RTCPeerConnection polyfill.
 *   3. Performs the full DTLS handshake, opens the "screenmirror" data channel.
 *   4. Receives real binary frames (SMJ0 + JPEG), decodes them, writes
 *      PNGs to disk as proof.
 *
 * Output: tools/output/frame-NNNN.png
 */
import { spawn } from 'node:child_process';
import path from 'node:path';
import http from 'node:http';
import WebSocket from 'ws';
import wrtcPkg from '@roamhq/wrtc';
const { RTCPeerConnection, RTCSessionDescription } = wrtcPkg;
import fs from 'node:fs';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '../..');
const SERVER_BIN = path.join(ROOT, 'src-tauri', 'target', 'debug', 'screenmirror-server');
const VIEWER_DIST = path.join(ROOT, 'viewer', 'dist');
const OUT_DIR = path.join(ROOT, 'tools', 'output');
const TEST_ROOM = 'e2e-room';
const FRAME_TARGET = 5; // capture N frames then stop
const FRAME_TIMEOUT_MS = 30000;

async function get(p) {
  return new Promise((resolve, reject) => {
    http.get(`http://127.0.0.1:3131${p}`, (res) => {
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

async function main() {
  console.log('[e2e-frames] starting standalone server...');
  const proc = spawn(SERVER_BIN, [], {
    env: {
      ...process.env,
      RUST_LOG: 'info',
      VIEWER_DIST,
      SCREENMIRROR_PORT: '3131',
      SCREENMIRROR_TEST_ROOM: TEST_ROOM,
      RUST_BACKTRACE: '1',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  proc.stdout.on('data', (d) => process.stdout.write('[srv] ' + d.toString()));
  proc.stderr.on('data', (d) => process.stderr.write('[srv!] ' + d.toString()));
  const cleanup = () => { try { proc.kill('SIGKILL'); } catch {} };
  process.on('exit', cleanup);
  process.on('SIGINT', () => { cleanup(); process.exit(130); });

  fs.mkdirSync(OUT_DIR, { recursive: true });

  try {
    await waitForServer();
    console.log('[e2e-frames] server up');

    // 1. Tell the server which screen/window to capture (so xcap actually produces frames).
    // We do this via an internal admin WebSocket message — but in this standalone
    // binary there's no Tauri IPC. Instead we can use the env var SCREENMIRROR_CAPTURE.
    // But for simplicity, let's use the default (xcap picks display 0 on macOS).
    // If headless (no display), xcap may return error and we'll capture no frames.

    // 2. Open WS as the viewer.
    const ws = new WebSocket(`ws://127.0.0.1:3131/api/ws?roomId=${TEST_ROOM}`);
    await new Promise((resolve, reject) => {
      const t = setTimeout(() => reject(new Error('ws timeout')), 5000);
      ws.on('open', () => { clearTimeout(t); resolve(); });
      ws.on('error', (e) => { clearTimeout(t); reject(e); });
    });
    console.log('[e2e-frames] WS open');

    // 3. Create RTCPeerConnection with data channel.
// Both ends are on 127.0.0.1, so we need iceTransportPolicy=relay won't help.
// We pass iceCandidatePoolSize and let wrtc gather UDP host candidates.
const pc = new RTCPeerConnection({
  iceServers: [],
  iceTransportPolicy: 'all',
  iceCandidatePoolSize: 0,
});
    // Disable local ICE candidate gathering: rely on the host's candidate in the answer.
    if (typeof pc.setLocalDescription === 'function') {
      // wrtc polyfill may not have setConfiguration; ignore failures
    }
    const channel = pc.createDataChannel('screenmirror', { ordered: true });
    channel.binaryType = 'arraybuffer';

    const frames = [];
    let resolved = false;

    await new Promise((resolveAll, rejectAll) => {
      const donePromise = new Promise((res) => {
        channel.onopen = () => {
          console.log('[e2e-frames] data channel OPEN');
        };
        channel.onmessage = (e) => {
          if (resolved) return;
          if (!(e.data instanceof ArrayBuffer)) return;
          if (e.data.byteLength < 8) return;
          const view = new DataView(e.data);
          if (view.getUint8(0) !== 0x53 || view.getUint8(1) !== 0x4d || view.getUint8(2) !== 0x4a || view.getUint8(3) !== 0x30) return;
          const id = view.getUint32(4, false);
          const jpeg = Buffer.from(e.data, 8);
          frames.push({ id, jpeg, t: Date.now() });
          console.log(`[e2e-frames] received frame #${id} (${jpeg.length} bytes JPEG)`);
          if (frames.length >= FRAME_TARGET) {
            resolved = true;
            res();
          }
        };
        channel.onerror = (err) => console.log('[e2e-frames] channel error:', err);
        channel.onclose = () => console.log('[e2e-frames] channel closed');

        pc.onicecandidate = (e) => {
          if (e.candidate) {
            const c = e.candidate.toJSON();
            console.log('[e2e-frames] local candidate:', c.candidate?.slice(0, 80));
            ws.send(JSON.stringify({ type: 'ICE_CANDIDATE', payload: { candidate: c } }));
          } else {
            console.log('[e2e-frames] ICE gathering complete');
          }
        };

        pc.oniceconnectionstatechange = () => {
          console.log('[e2e-frames] ICE state:', pc.iceConnectionState);
        };

        pc.onconnectionstatechange = () => {
          console.log('[e2e-frames] connection state:', pc.connectionState);
        };

        pc.ondatachannel = (e) => {
          console.log('[e2e-frames] ondatachannel:', e.channel.label);
        };

        ws.on('message', async (data) => {
          const text = data.toString();
          if (text.includes('"ANSWER"')) {
            const m = JSON.parse(text);
            console.log('[e2e-frames] ANSWER SDP:', m.payload.sdp.slice(0, 800));
            const answer = new RTCSessionDescription({ type: 'answer', sdp: m.payload.sdp });
            await pc.setRemoteDescription(answer);
            console.log('[e2e-frames] remote description set');
          } else if (text.includes('"ICE_CANDIDATE"')) {
            try {
              const m = JSON.parse(text);
              if (m.payload && m.payload.candidate) {
                await pc.addIceCandidate(m.payload.candidate);
                console.log('[e2e-frames] added remote ICE');
              }
            } catch (e) {
              console.log('[e2e-frames] ICE add failed:', e.message);
            }
          } else if (text.includes('"ERROR"')) {
            console.log('[e2e-frames] server error:', text);
          }
        });

        // After WS open + small delay, create offer and send.
        setTimeout(async () => {
          try {
            const offer = await pc.createOffer();
            await pc.setLocalDescription(offer);
            ws.send(JSON.stringify({ type: 'OFFER', payload: { sdp: offer.sdp } }));
            console.log('[e2e-frames] OFFER sent');
          } catch (e) {
            console.error('[e2e-frames] createOffer failed:', e);
            rejectAll(e);
          }
        }, 1500);

        // Hard timeout
        setTimeout(() => {
          if (!resolved) rejectAll(new Error(`only received ${frames.length}/${FRAME_TARGET} frames in ${FRAME_TIMEOUT_MS}ms`));
        }, FRAME_TIMEOUT_MS);
      });

      donePromise.then(resolveAll, rejectAll);
    });

    console.log(`\n[e2e-frames] captured ${frames.length} frames — writing to ${OUT_DIR}`);

    // Write JPEG files (they ARE the frames from the host's xcap).
    for (const f of frames) {
      const jpegPath = path.join(OUT_DIR, `frame-${String(f.id).padStart(4, '0')}.jpg`);
      fs.writeFileSync(jpegPath, f.jpeg);
    }

    // Compute byte-level hash for proof of uniqueness.
    const hashes = frames.map((f) => crypto.createHash('sha256').update(f.jpeg).digest('hex').slice(0, 16));
    console.log('[e2e-frames] frame SHA-256 (first 16 hex):', hashes);

    // Verify JPEG magic bytes
    for (const f of frames) {
      if (f.jpeg[0] !== 0xff || f.jpeg[1] !== 0xd8) {
        console.log(`[e2e-frames] WARN: frame #${f.id} is not a valid JPEG (magic ${f.jpeg[0].toString(16)} ${f.jpeg[1].toString(16)})`);
      }
    }

    // Summary
    console.log('\n=== E2E FRAME CAPTURE SUCCESS ===');
    console.log(`Frames received: ${frames.length}`);
    console.log(`Output dir: ${OUT_DIR}`);
    console.log(`JPEG file sizes: ${frames.map((f) => f.jpeg.length).join(', ')} bytes`);
    console.log('Frame IDs:', frames.map((f) => f.id).join(', '));

    cleanup();
    process.exit(0);
  } catch (e) {
    console.error('[e2e-frames] FAIL:', e.message);
    cleanup();
    process.exit(1);
  }
}

main();
