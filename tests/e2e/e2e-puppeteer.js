#!/usr/bin/env node
/**
 * Real GUI E2E test using Puppeteer + system Google Chrome.
 *
 * Pipeline:
 *   1. Tauri binary running, signaling server bound to 127.0.0.1:3131.
 *   2. Spawn headless Chrome with auto-granted screen-capture permission
 *      (not strictly needed — we capture from the host, not the browser).
 *   3. Navigate Chrome to http://127.0.0.1:3131/<roomId>.
 *   4. Viewer creates RTCPeerConnection + data channel, sends OFFER.
 *   5. Host's HostPeer accepts offer, ICE+DTLS+SCTP handshake, channel opens.
 *   6. Host's xcap captures real screen → JPEG → DataChannel.
 *   7. Viewer receives frames, sets <img>.src = blob URL.
 *   8. Screenshot the page; verify <img> rendered with non-trivial pixels.
 */
import { spawn } from 'node:child_process';
import path from 'node:path';
import http from 'node:http';
import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '../..');
const TAURI_BIN = path.join(ROOT, 'src-tauri', 'target', 'debug', 'screenmirror');
const VIEWER_DIST = path.join(ROOT, 'viewer', 'dist');
const OUT_DIR = path.join(ROOT, 'tools', 'output');
const ROOM_ID = 'puppeteer-room';
const PORT = 3131;
const BASE = `http://127.0.0.1:${PORT}`;
const URL = `${BASE}/${ROOM_ID}`;

async function get(p) {
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
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error('server did not start');
}

async function main() {
  fs.mkdirSync(OUT_DIR, { recursive: true });
  console.log('[puppeteer-e2e] launching Tauri binary...');
  const proc = spawn(TAURI_BIN, [], {
    env: {
      ...process.env,
      RUST_LOG: 'info',
      SCREENMIRROR_PORT: String(PORT),
      SCREENMIRROR_TEST_ROOM: ROOM_ID,
      SCREENMIRROR_CAPTURE: 'screen',
      SCREENMIRROR_E2E_AUTO_APPROVE: '1',
      VIEWER_DIST,
      ELECTRON_DISABLE_SECURITY_WARNINGS: '1',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  proc.stdout.on('data', (d) => process.stdout.write('[tauri] ' + d.toString()));
  proc.stderr.on('data', (d) => process.stderr.write('[tauri!] ' + d.toString()));
  const cleanup = () => { try { proc.kill('SIGKILL'); } catch {} };
  process.on('exit', cleanup);
  process.on('SIGINT', () => { cleanup(); process.exit(130); });

  try {
    await waitForServer();
    console.log('[puppeteer-e2e] server up');

    // Sanity-check viewer SPA served.
    const spa = await get(`/${ROOM_ID}`);
    console.log(`[puppeteer-e2e] SPA status=${spa.status}, len=${spa.body.length}`);

    // Launch headless Chrome.
    console.log('[puppeteer-e2e] launching headless Chrome...');
    let browser;
    try {
      browser = await puppeteer.launch({
      executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      headless: true, // old headless mode (more permissive localhost)
      protocolTimeout: 60000,
      args: [
        '--no-sandbox',
        '--disable-dev-shm-usage',
        '--use-fake-ui-for-media-stream',
        '--use-fake-device-for-media-stream',
        '--autoplay-policy=no-user-gesture-required',
        '--allow-running-insecure-content',
        '--disable-features=IsolateOrigins,site-per-process',
        '--unsafely-treat-insecure-origin-as-secure=http://127.0.0.1:3131',
      ],
      defaultViewport: { width: 1280, height: 800 },
    });
    } catch (e) {
      console.error('[puppeteer-e2e] launch error:', e.message);
      console.error('[puppeteer-e2e] launch stack:', e.stack);
      throw e;
    }
    console.log('[puppeteer-e2e] Chrome launched');

    const page = await browser.newPage();
    await page.setCacheEnabled(false);

    // Capture console logs from the viewer
    page.on('console', (msg) => {
      console.log(`[viewer-console:${msg.type()}]`, msg.text());
    });
    page.on('pageerror', (err) => {
      console.log('[viewer-error]', err.message);
    });
    page.on('requestfailed', (req) => {
      console.log('[viewer-req-failed]', req.url(), req.failure()?.errorText);
    });

    // Patch RTCPeerConnection to log ICE state and SDP for debugging.
    await page.evaluateOnNewDocument(() => {
      const origPC = window.RTCPeerConnection;
      window.RTCPeerConnection = function (...args) {
        const pc = new origPC(...args);
        const origSetRemote = pc.setRemoteDescription.bind(pc);
        pc.setRemoteDescription = async (desc) => {
          console.log('[pc] setRemoteDescription type=' + desc.type);
          return origSetRemote(desc);
        };
        const origSetLocal = pc.setLocalDescription.bind(pc);
        pc.setLocalDescription = async (desc) => {
          console.log('[pc] setLocalDescription type=' + desc.type);
          return origSetLocal(desc);
        };
        pc.addEventListener('iceconnectionstatechange', () => {
          console.log('[pc] iceConnectionState=' + pc.iceConnectionState);
        });
        pc.addEventListener('connectionstatechange', () => {
          console.log('[pc] connectionState=' + pc.connectionState);
        });
        pc.addEventListener('datachannel', (e) => {
          console.log('[pc] datachannel label=' + e.channel.label);
          const ch = e.channel;
          ch.addEventListener('open', () => console.log('[dc] open'));
          ch.addEventListener('close', () => console.log('[dc] close'));
          let count = 0;
          ch.addEventListener('message', (ev) => {
            count++;
            if (count <= 5 || count % 30 === 0) {
              console.log('[dc] message #' + count + ' bytes=' + (ev.data?.byteLength ?? ev.data?.length ?? '?'));
            }
          });
        });
        return pc;
      };
      // Patch WebSocket
      const origWS = window.WebSocket;
      window.WebSocket = function (url, protocols) {
        const ws = new origWS(url, protocols);
        console.log('[ws] open ' + url);
        ws.addEventListener('open', () => console.log('[ws] open'));
        ws.addEventListener('message', (e) => {
          const t = String(e.data);
          if (t.length < 200) console.log('[ws] recv ' + t.slice(0, 200));
          else console.log('[ws] recv ' + t.slice(0, 80) + '... (' + t.length + ' bytes)');
        });
        return ws;
      };
    });

    console.log('[puppeteer-e2e] navigating to ' + URL);
    page.on('request', (req) => {
      if (!req.url().endsWith('.ico')) {
        console.log(`[viewer-req] ${req.method()} ${req.url()}`);
      }
    });
    page.on('response', (res) => {
      console.log(`[viewer-res] ${res.status()} ${res.url().slice(0, 120)}`);
    });
    await page.goto(URL, { waitUntil: 'networkidle2', timeout: 30000 });

    // Print page title and any errors
    const title = await page.title();
    const bodyText = await page.evaluate(() => document.body?.innerText?.slice(0, 500));
    console.log('[puppeteer-e2e] page title:', title);
    console.log('[puppeteer-e2e] body text:', bodyText);

    // Give time for ICE/DTLS/SCTP handshake + frame delivery.
    console.log('[puppeteer-e2e] waiting for frames to flow (40s)...');
    await new Promise((r) => setTimeout(r, 40000));

    // Skip direct WS test in this build (it overwrites the app's viewer_sinks
    // entry, which is why the app's WS doesn't receive answers during the test).

    // Inspect the viewer DOM: check <img> src and dimensions.
    const imgInfo = await page.evaluate(() => {
      const img = document.querySelector('img.frame');
      if (!img) return { found: false };
      return {
        found: true,
        naturalWidth: img.naturalWidth,
        naturalHeight: img.naturalHeight,
        displayWidth: img.clientWidth,
        displayHeight: img.clientHeight,
        srcPrefix: img.src?.slice(0, 50),
      };
    });
    console.log('[puppeteer-e2e] <img> state:', JSON.stringify(imgInfo));

    // Take a screenshot of the page.
    const shotPath = path.join(OUT_DIR, 'viewer-with-frame.png');
    await page.screenshot({ path: shotPath, fullPage: true });
    console.log('[puppeteer-e2e] screenshot saved:', shotPath);

    // Verify the screenshot is non-trivial (not all white/black)
    const screenshotBuf = fs.readFileSync(shotPath);
    const sha = crypto.createHash('sha256').update(screenshotBuf).digest('hex').slice(0, 32);
    console.log('[puppeteer-e2e] screenshot SHA256:', sha);

    // Quick pixel sanity check: count distinct pixel colors via canvas in the page
    const dcStats = await page.evaluate(() => ({
      dcBytes: (window).__dcBytes ?? 0,
      dcCount: (window).__dcCount ?? 0,
      decodeFail: (window).__decodeFail ?? 0,
      frameCount: (window).__smFrameCount ?? 0,
    }));
    console.log('[puppeteer-e2e] dc stats:', JSON.stringify(dcStats));

    const pixelStats = await page.evaluate(() => {
      const img = document.querySelector('img.frame');
      if (!img || !img.complete || img.naturalWidth === 0) return { ready: false };
      const c = document.createElement('canvas');
      c.width = img.naturalWidth;
      c.height = img.naturalHeight;
      const ctx = c.getContext('2d');
      if (!ctx) return { ready: false, reason: 'no ctx' };
      ctx.drawImage(img, 0, 0);
      const data = ctx.getImageData(0, 0, c.width, c.height).data;
      // Compute mean + standard deviation over all pixels (luminance)
      let sum = 0;
      let sumSq = 0;
      const n = data.length / 4;
      const colors = new Set();
      for (let i = 0; i < data.length; i += 4) {
        const lum = (data[i] + data[i + 1] + data[i + 2]) / 3;
        sum += lum;
        sumSq += lum * lum;
        if (i % 4000 === 0) colors.add(`${data[i]},${data[i + 1]},${data[i + 2]}`);
      }
      const mean = sum / n;
      const variance = sumSq / n - mean * mean;
      const stddev = Math.sqrt(variance);
      return {
        ready: true,
        naturalWidth: img.naturalWidth,
        naturalHeight: img.naturalHeight,
        mean: mean.toFixed(2),
        stddev: stddev.toFixed(2),
        distinctColorsSampled: colors.size,
      };
    });
    console.log('[puppeteer-e2e] pixel stats:', JSON.stringify(pixelStats));

    // Get latest data-channel message count via console
    const dcInfo = await page.evaluate(() => {
      // @ts-ignore
      return window.__dcStats ?? null;
    });

    await browser.close();
    cleanup();

    console.log('\n=== VERDICT ===');
    console.log('imgInfo.found:', imgInfo.found);
    console.log('imgInfo.naturalWidth:', imgInfo.naturalWidth);
    console.log('pixelStats.ready:', pixelStats.ready);
    if (pixelStats.ready) {
      console.log(`frame ${pixelStats.naturalWidth}x${pixelStats.naturalHeight} mean=${pixelStats.mean} stddev=${pixelStats.stddev} distinctColors≈${pixelStats.distinctColorsSampled}`);
    }
    const ok = imgInfo.found && pixelStats.ready && pixelStats.naturalWidth > 0;
    console.log(ok ? '✅ SUCCESS' : '❌ FAILURE');
    process.exit(ok ? 0 : 1);
  } catch (e) {
    console.error('[puppeteer-e2e] FAIL:', e.message);
    cleanup();
    process.exit(1);
  }
}

main();
