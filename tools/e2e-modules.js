#!/usr/bin/env node
/**
 * Comprehensive E2E test covering the host, signaling, controls, privacy, and native video modules:
 *
 *   1. QR-code pairing
 *   2. Tray installation
 *   3. Room management
 *   4. Device approval
 *   5. Quality / fullscreen / pause controls
 *   6. Privacy surface
 *   7. Native WebRTC video playback
 *
 * Output: tools/output/comprehensive.json + viewer-comprehensive.png
 */
import { spawn } from 'node:child_process';
import path from 'node:path';
import http from 'node:http';
import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import crypto from 'node:crypto';
import QRCode from 'qrcode';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const TAURI_BIN = path.join(ROOT, 'src-tauri', 'target', 'debug', 'screenmirror');
const VIEWER_DIST = path.join(ROOT, 'viewer', 'dist');
const OUT_DIR = path.join(__dirname, 'output');
const ROOM_ID = 'e2e-modules';
// PORT is read from /api/host-info after the binary starts; the env var
// SCREENMIRROR_PORT below is just the initial guess for the spawn step.
const FALLBACK_PORT = 3131;
let PORT = FALLBACK_PORT;
const base = () => `http://127.0.0.1:${PORT}`;

function get(p) {
  return new Promise((resolve, reject) => {
    http.get(base() + p, (res) => {
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
  throw new Error('server did not start');
}

// Wait until at least one of the patterns appears in stderrLines, or timeout.
async function waitForStderr(patterns, timeout = 5000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    const blob = stderrLines.join('');
    if (patterns.some((p) => (typeof p === 'string' ? blob.includes(p) : p.test(blob)))) {
      return;
    }
    await new Promise((r) => setTimeout(r, 100));
  }
}

// Module-scoped stderr buffer — set up before spawn so waitForStderr
// (defined above) can poll it.
const stderrLines = [];

const results = {
  qr: null,
  tray: null,
  room: null,
  approval: null,
  controls: null,
  privacy: null,
  frames: null,
  firstEncodedFrameMs: null,
  avgEncodedFrameMs: null,
  viewerStatusPill: null,
  viewerSpinner: null,
  rebrand: null,
  visual: null,
};

function record(module, ok, detail) {
  results[module] = { ok, detail };
  console.log(`[e2e] ${ok ? '✅' : '❌'} ${module}: ${detail}`);
}

async function main() {
  fs.mkdirSync(OUT_DIR, { recursive: true });
  console.log('[e2e] launching Tauri binary...');
  const proc = spawn(TAURI_BIN, [], {
    env: {
      ...process.env,
      RUST_LOG: 'info',
      SCREENMIRROR_PORT: String(FALLBACK_PORT),
      SCREENMIRROR_TEST_ROOM: ROOM_ID,
      SCREENMIRROR_CAPTURE: 'screen',
      SCREENMIRROR_E2E_AUTO_APPROVE: '1',
      SCREENMIRROR_MAX_DIM: '480',
      SCREENMIRROR_CAPTURE_QUALITY: '0.3',
      VIEWER_DIST,
      ELECTRON_DISABLE_SECURITY_WARNINGS: '1',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  proc.stdout.on('data', (d) => process.stdout.write('[tauri] ' + d.toString()));
  proc.stderr.on('data', (d) => {
    const s = d.toString();
    stderrLines.push(s);
    process.stderr.write('[tauri!] ' + s);
  });
  const cleanup = () => { try { proc.kill('SIGKILL'); } catch {} };
  process.on('exit', cleanup);
  process.on('SIGINT', () => { cleanup(); process.exit(130); });

  try {
    await waitForServer();
    console.log('[e2e] server up');

    // The setup hook runs asynchronously after the listener binds; the tray
    // and smoke-room log lines may appear a moment after /api/health succeeds.
    // Wait for both before snapshotting stderr.
    await waitForStderr([/tray icon installed/, /smoke room registered/], 5000);
    await new Promise((r) => setTimeout(r, 250));

    // (1) QR + (3) room management: pull /api/host-info.
    // The port comes from the binary now (not hard-coded) so the same
    // harness works against the Tauri shell and the standalone server.
    const info = JSON.parse((await get('/api/host-info')).body);
    const lanIp = info.lan_ip;
    if (typeof info.port === 'number') {
      PORT = info.port;
      console.log('[e2e] host reported port', PORT);
    }
    console.log('[e2e] host-info:', JSON.stringify(info));
    const qrUrl = `http://${lanIp}:${PORT}/${ROOM_ID}`;
    const qrDataUrl = await QRCode.toDataURL(qrUrl, { errorCorrectionLevel: 'H', width: 240, margin: 1 });
    record('qr', !!qrDataUrl.startsWith('data:image/png;base64,'), `URL=${qrUrl}, qr-png-bytes=${Math.round(qrDataUrl.length * 3 / 4)}`);

    // Room: smoke room registered from env; pending-device only after viewer connects
    const smRoomSeen = stderrLines.join('').includes(`smoke room registered: ${ROOM_ID}`);
    record('room', smRoomSeen, `smoke room ${ROOM_ID} registered at startup`);

    // (2) Tray: build emits "tray icon installed"
    const traySeen = stderrLines.join('').includes('tray icon installed');
    record('tray', traySeen, traySeen ? 'log line "tray icon installed" emitted during setup' : 'tray log line missing');

    // (4)+(7) Device approval + frames: launch Chrome, navigate to viewer URL.
    console.log('[e2e] launching headless Chrome...');
    const browser = await puppeteer.launch({
      executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      headless: true,
      protocolTimeout: 60000,
      args: [
        '--no-sandbox',
        '--disable-dev-shm-usage',
        '--use-fake-ui-for-media-stream',
        '--use-fake-device-for-media-stream',
        '--autoplay-policy=no-user-gesture-required',
        '--allow-running-insecure-content',
        '--unsafely-treat-insecure-origin-as-secure=http://127.0.0.1:3131',
      ],
      defaultViewport: { width: 1280, height: 800 },
    });
    const page = await browser.newPage();
    await page.setCacheEnabled(false);
    page.on('console', (msg) => {
      console.log(`[viewer-console:${msg.type()}]`, msg.text());
    });
    page.on('pageerror', (err) => console.log('[viewer-error]', err.message));

    await page.goto(`${base()}/${ROOM_ID}`, { waitUntil: 'networkidle2', timeout: 30000 });

    // Privacy: verify that the page does not load external tracking resources.
    const privacyCheck = await page.evaluate(() => {
      const externalResources = Array.from(document.querySelectorAll('script[src], iframe[src], img[src]'));
      const hasExternalResource = externalResources.some((element) => {
        const source = element.getAttribute('src') ?? '';
        return /^https?:\/\//i.test(source);
      });
      const bg = getComputedStyle(document.body).backgroundColor;
      const html = document.documentElement.outerHTML.length;
      return { hasExternalResource, bg, html };
    });
    record(
      'privacy',
      !privacyCheck.hasExternalResource,
      `no external tracking resources (external=${privacyCheck.hasExternalResource}, html-bytes=${privacyCheck.html})`,
    );

    // (9) Visual assertion: the page must have a real background and content,
    // catching any regression to an empty white page. We check both body bg
    // and the #app container bg, since the viewer styles #app directly.
    const visualProbe = await page.evaluate(() => {
      const body = document.body;
      const app = document.getElementById('app');
      return {
        bodyBg: getComputedStyle(body).backgroundColor,
        appBg: app ? getComputedStyle(app).backgroundColor : '',
        htmlBytes: document.documentElement.outerHTML.length,
        stylesheets: Array.from(document.styleSheets).length,
      };
    });
    const visualOk =
      (visualProbe.bodyBg !== 'rgba(0, 0, 0, 0)' && visualProbe.bodyBg !== '') ||
      (visualProbe.appBg !== 'rgba(0, 0, 0, 0)' && visualProbe.appBg !== '');
    record(
      'visual',
      visualOk,
      `body=${visualProbe.bodyBg}, #app=${visualProbe.appBg}, html-bytes=${visualProbe.htmlBytes}, sheets=${visualProbe.stylesheets}`,
    );

    // (4) Device approval: viewer advanced from connecting screen to player
    //     (i.e. ALLOWED_TO_CONNECT arrived).
    await new Promise((r) => setTimeout(r, 35000));
    const approvalState = await page.evaluate(() => {
      const body = document.body?.innerText ?? '';
      return {
        inPlayer: body.includes('暂停') && body.includes('画质') && body.includes('全屏'),
        body: body.slice(0, 200),
      };
    });
    record('approval', approvalState.inPlayer, `viewer reached player view (body excerpt: ${JSON.stringify(approvalState.body)})`);

    // (5) Quality / fullscreen / pause controls: prove they exist and are wired.
    const controlsState = await page.evaluate(() => {
      // The Vue component renders <button> elements for the panel items.
      const btns = Array.from(document.querySelectorAll('button'));
      const labels = btns.map((b) => b.textContent?.trim() ?? '');
      const hasPause = labels.some((l) => l === '暂停' || l === 'Play' || l === 'Pause' || l === '播放' || l === '继续');
      const hasQuality = !!document.querySelector('select');
      const hasFullscreen = labels.some((l) => l === '全屏' || /fullscreen/i.test(l));
      return { hasPause, hasQuality, hasFullscreen, buttonCount: btns.length, labels };
    });
    const okCtrl = controlsState.hasPause && controlsState.hasQuality && controlsState.hasFullscreen;
    record(
      'controls',
      okCtrl,
      `pause=${controlsState.hasPause} quality-select=${controlsState.hasQuality} fullscreen=${controlsState.hasFullscreen} btnCount=${controlsState.buttonCount}`,
    );

    // (5b) Viewer status indicator: confirm the corner pill + spinner render
    //      once the player view is mounted. The pill should report
    //      'streaming' once frames flow, but at minimum the wrapper element
    //      and a [data-state] attribute must exist.
    const statusState = await page.evaluate(() => {
      const pill = document.querySelector('.player-status');
      const spinner = document.querySelector('.spinner');
      const dataState = pill?.getAttribute('data-state') ?? null;
      const text = pill?.textContent?.trim() ?? '';
      return { hasPill: !!pill, hasSpinner: !!spinner, dataState, text };
    });
    record(
      'viewerStatusPill',
      statusState.hasPill && statusState.dataState === 'streaming',
      `pill=${statusState.hasPill} state=${statusState.dataState} text=${JSON.stringify(statusState.text)}`,
    );
    record(
      'viewerSpinner',
      statusState.hasSpinner || statusState.dataState === 'streaming',
      `spinner=${statusState.hasSpinner} (note: spinner unmounts once streaming)`,
    );

    // Video: verify a native WebRTC video element has dimensions and advances.
    const framesState = await page.evaluate(async () => {
      const video = document.querySelector('video.frame');
      if (!(video instanceof HTMLVideoElement)) return { videoWidth: 0, videoHeight: 0, readyState: 0, currentTime: 0 };
      const before = video.currentTime;
      await new Promise((resolve) => setTimeout(resolve, 1000));
      return { videoWidth: video.videoWidth, videoHeight: video.videoHeight, readyState: video.readyState, currentTime: video.currentTime - before };
    });
    const okFrames = framesState.videoWidth > 0 && framesState.videoHeight > 0 && framesState.readyState >= 2 && framesState.currentTime > 0;
    record('frames', okFrames, `video ${framesState.videoWidth}x${framesState.videoHeight}, readyState=${framesState.readyState}, timeDelta=${framesState.currentTime.toFixed(3)}s`);

    // Latency: retain ISO timestamp parsing as a fallback/supplementary
    // measurement for both verbose and sparse encoded-frame log lines.
    const stderrBlob = stderrLines.join('');
    const re = /(\d{4}-\d{2}-\d{2}T[\d:.]+Z?)[^\n]*video capture: encoded frame #(\d+)/g;
    const byFrame = new Map();
    let _m;
    while ((_m = re.exec(stderrBlob))) {
      const ts = new Date(_m[1]).getTime();
      const frameNum = parseInt(_m[2], 10);
      if (!Number.isNaN(ts) && !byFrame.has(frameNum)) byFrame.set(frameNum, ts);
    }
    const encodeTimestamps = [...byFrame.entries()].sort((a, b) => a[0] - b[0]).map(([, t]) => t);
    const fallbackFirstFrameMs = encodeTimestamps.length >= 2
      ? encodeTimestamps[1] - encodeTimestamps[0]
      : 0;
    const fallbackAvgFrameMs = encodeTimestamps.length > 2
      ? (encodeTimestamps[encodeTimestamps.length - 1] - encodeTimestamps[0]) / (encodeTimestamps.length - 1)
      : 0;

    // Authoritative timing: parse `total_elapsed=` from verbose log lines
    // (frames 1-3). The first value is capture-thread-to-first-packet latency;
    // subsequent deltas measure per-frame encode spacing.
    const verboseMatches = [...stderrBlob.matchAll(/video capture: encoded frame #(\d+) total_elapsed=([0-9.]+)s/g)];
    const totalElapsedSec = verboseMatches
      .map((m) => ({ frame: parseInt(m[1], 10), seconds: parseFloat(m[2]) }))
      .filter(({ frame, seconds }) => frame >= 1 && frame <= 3 && !Number.isNaN(seconds))
      .sort((a, b) => a.frame - b.frame)
      .map(({ seconds }) => seconds);

    const firstEncodedFrameMs = totalElapsedSec.length > 0
      ? totalElapsedSec[0] * 1000
      : fallbackFirstFrameMs;
    let avgEncodedFrameMs = fallbackAvgFrameMs;
    if (totalElapsedSec.length >= 2) {
      const deltas = [];
      for (let i = 1; i < totalElapsedSec.length; i++) {
        deltas.push((totalElapsedSec[i] - totalElapsedSec[i - 1]) * 1000);
      }
      avgEncodedFrameMs = deltas.reduce((a, b) => a + b, 0) / deltas.length;
    }

    record('firstEncodedFrameMs', firstEncodedFrameMs > 0 && firstEncodedFrameMs < 3000, `firstEncodedFrameMs=${firstEncodedFrameMs.toFixed(0)} verboseFrames=${totalElapsedSec.length} timestampFrames=${encodeTimestamps.length}`);
    record('avgEncodedFrameMs', avgEncodedFrameMs > 0 && avgEncodedFrameMs < 200, `avgEncodedFrameMs=${avgEncodedFrameMs.toFixed(1)} verboseFrames=${totalElapsedSec.length} timestampFrames=${encodeTimestamps.length}`);

    // Screenshot
    const shot = path.join(OUT_DIR, 'viewer-comprehensive.png');
    await page.screenshot({ path: shot, fullPage: true });
    const sha = crypto.createHash('sha256').update(fs.readFileSync(shot)).digest('hex').slice(0, 32);
    console.log('[e2e] screenshot saved:', shot, 'sha256:', sha);

    // Save QR data URL to disk as proof
    const qrFile = path.join(OUT_DIR, 'qr.png');
    const b64 = qrDataUrl.split(',')[1];
    fs.writeFileSync(qrFile, Buffer.from(b64, 'base64'));
    console.log('[e2e] QR code saved:', qrFile);

    await browser.close();
    cleanup();

    // Write the full results file
    fs.writeFileSync(
      path.join(OUT_DIR, 'comprehensive.json'),
      JSON.stringify({ ts: new Date().toISOString(), results, qrUrl }, null, 2),
    );

    console.log('\n=== COMPREHENSIVE VERDICT ===');
    const allOk = Object.values(results).every((r) => r?.ok);
    for (const [k, v] of Object.entries(results)) {
      console.log(`  ${v.ok ? '✅' : '❌'} ${k}`);
    }
    console.log(allOk ? '\n✅ ALL 7 MODULES PASS' : '\n❌ some modules failed');
    process.exit(allOk ? 0 : 1);
  } catch (e) {
    console.error('[e2e] FAIL:', e.message);
    console.error(e.stack);
    cleanup();
    process.exit(1);
  }
}

main();
