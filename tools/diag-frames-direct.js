#!/usr/bin/env node
/**
 * Direct diagnostic: bypass puppeteer's auto-launch (which is failing on
 * macOS Chrome 149 headless), and use puppeteer.connect() against a Chrome
 * we launch ourselves with --user-data-dir.
 *
 * Goals:
 *   1. Start real Tauri host.
 *   2. Launch Chrome ourselves (headless, with fresh user-data-dir).
 *   3. Connect puppeteer to it.
 *   4. Open the viewer page, capture EVERY console line + state, until
 *      either frames render or the 5s watchdog fires.
 *   5. Print a structured trace so we can pinpoint the actual root cause
 *      instead of guessing from the symptom.
 */
import { spawn, spawnSync } from 'node:child_process';
import path from 'node:path';
import http from 'node:http';
import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const TAURI_BIN = process.env.SCREENMIRROR_TAURI_BIN
  ?? path.join(ROOT, 'src-tauri', 'target', 'debug', 'screenmirror');
const VIEWER_DIST = path.join(ROOT, 'viewer', 'dist');
const OUT_DIR = path.join(__dirname, 'output');
const ROOM_ID = 'diag-direct';
const PORT = Number(process.env.SCREENMIRROR_DIAG_PORT ?? 3131);
const BASE = `http://127.0.0.1:${PORT}`;
const CHROME_BIN = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const DEBUG_PORT = Number(process.env.SCREENMIRROR_DIAG_CHROME_DEBUG_PORT ?? 9444);
const USER_DATA_DIR = `/tmp/screenmirror-chrome-${Date.now()}`;

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
  throw new Error('server did not start');
}

async function waitForChrome(timeout = 30000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    try {
      const r = await new Promise((resolve, reject) => {
        http.get(`http://127.0.0.1:${DEBUG_PORT}/json/version`, (res) => {
          let body = '';
          res.on('data', (c) => (body += c));
          res.on('end', () => resolve(JSON.parse(body)));
        }).on('error', reject);
      });
      return r;
    } catch {}
    await new Promise((r) => setTimeout(r, 300));
  }
  throw new Error('chrome did not expose debug port');
}

async function main() {
  fs.mkdirSync(OUT_DIR, { recursive: true });
  console.log('[diag] launching Tauri binary...');
  const tauri = spawn(TAURI_BIN, ['--port', String(PORT)], {
    env: {
      ...process.env,
      RUST_LOG: 'info',
      SCREENMIRROR_PORT: String(PORT),
      SCREENMIRROR_TEST_ROOM: ROOM_ID,
      SCREENMIRROR_CAPTURE: process.env.SCREENMIRROR_CAPTURE ?? 'test',
      SCREENMIRROR_E2E_AUTO_APPROVE: '1',
      // Keep the diagnostic aligned with the selected product profile. Use
      // explicit env overrides when a smaller/faster test pattern is wanted.
      SCREENMIRROR_MAX_DIM: process.env.SCREENMIRROR_MAX_DIM ?? '2560',
      SCREENMIRROR_CAPTURE_QUALITY: process.env.SCREENMIRROR_CAPTURE_QUALITY ?? '0.75',
      VIEWER_DIST,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  tauri.stdout.on('data', (d) => process.stdout.write('[tauri] ' + d.toString()));
  tauri.stderr.on('data', (d) => process.stderr.write('[tauri!] ' + d.toString()));

  let cleaned = false;
  const killTauri = () => {
    if (cleaned) return;
    cleaned = true;
    try { tauri.kill('SIGKILL'); } catch {}
  };
  process.on('exit', killTauri);
  process.on('SIGINT', () => { killTauri(); process.exit(130); });
  process.on('SIGTERM', () => { killTauri(); process.exit(143); });

  let chromeProc = null;
  const killChrome = () => {
    if (chromeProc && chromeProc.exitCode === null) {
      try { chromeProc.kill('SIGKILL'); } catch {}
    }
    try { fs.rmSync(USER_DATA_DIR, { recursive: true, force: true }); } catch {}
  };
  process.on('exit', killChrome);

  try {
    await waitForServer();
    console.log('[diag] server up');

    console.log('[diag] launching Chrome...');
    chromeProc = spawn(
      CHROME_BIN,
      [
        '--headless=new',
        '--no-sandbox',
        '--disable-gpu',
        '--disable-dev-shm-usage',
        '--use-fake-ui-for-media-stream',
        '--use-fake-device-for-media-stream',
        '--autoplay-policy=no-user-gesture-required',
        '--allow-running-insecure-content',
        `--unsafely-treat-insecure-origin-as-secure=${BASE}`,
        `--remote-debugging-port=${DEBUG_PORT}`,
        `--user-data-dir=${USER_DATA_DIR}`,
      ],
      { stdio: ['ignore', 'pipe', 'pipe'] },
    );
    chromeProc.stdout.on('data', (d) => process.stdout.write('[chrome] ' + d.toString()));
    chromeProc.stderr.on('data', (d) => process.stderr.write('[chrome!] ' + d.toString()));

    const version = await waitForChrome();
    console.log(`[diag] chrome up: ${version.Browser}`);

    const browser = await puppeteer.connect({
      browserURL: `http://127.0.0.1:${DEBUG_PORT}`,
      protocolTimeout: 60000,
    });
    const page = await browser.newPage();
    await page.setCacheEnabled(false);

    page.on('console', (msg) => {
      const t = msg.text();
      console.log(`[viewer:${msg.type()}]`, t.slice(0, 400));
    });
    page.on('pageerror', (err) => console.log('[viewer-error]', err.message, err.stack));
    page.on('requestfailed', (req) => {
      console.log(`[viewer-req-fail] ${req.method()} ${req.url()} - ${req.failure()?.errorText}`);
    });

    const t0 = Date.now();
    console.log(`[diag] navigating to ${BASE}/${ROOM_ID}`);
    await page.goto(`${BASE}/${ROOM_ID}`, { waitUntil: 'networkidle2', timeout: 30000 });
    console.log(`[diag] page loaded at +${Date.now() - t0}ms`);
    spawnSync('osascript', ['-e', 'tell application "System Events" to tell process "Clock" to set frontmost to true']);
    await new Promise((resolve) => setTimeout(resolve, 500));
    let baseline = null;
    const baselineDeadline = Date.now() + 8000;
    while (!baseline && Date.now() < baselineDeadline) {
      baseline = await page.evaluate(() => {
        const v = document.querySelector('video.frame');
        if (!v || v.videoWidth === 0) return null;
        const c = document.createElement('canvas');
        c.width = v.videoWidth;
        c.height = v.videoHeight;
        const ctx = c.getContext('2d');
        ctx.drawImage(v, 0, 0, c.width, c.height);
        const data = ctx.getImageData(0, 0, c.width, c.height).data;
        let signature = 0;
        for (let i = 0; i < data.length; i += 64) signature = (signature + data[i] * 3 + data[i + 1] * 5 + data[i + 2] * 7) >>> 0;
        return { signature };
      });
      if (!baseline) await new Promise((resolve) => setTimeout(resolve, 200));
    }
    console.log(`[diag] baseline signature=${baseline?.signature ?? 'unavailable'} at +${Date.now() - t0}ms`);
    const changeTriggeredAt = Date.now() - t0;
    console.log(`[diag] waiting for live Clock stopwatch change at +${changeTriggeredAt}ms`);

    // Poll every 200ms for 15s — capture state evolution
    const trace = [];
    let pixelSample = null;
    let firstChangedAt = 0;
    while (Date.now() - t0 < 15000) {
      const snap = await page.evaluate(async () => {
        const v = document.querySelector('video.frame');
        const snap = {
          hasVideo: !!v,
          videoWidth: v?.videoWidth ?? 0,
          videoHeight: v?.videoHeight ?? 0,
          readyState: v?.readyState ?? -1,
          networkState: v?.networkState ?? -1,
          errorCode: v?.error?.code ?? null,
          paused: v?.paused ?? null,
          currentTime: v?.currentTime ?? 0,
          srcObject: v?.srcObject ? 'yes' : 'no',
          statusPill: document.querySelector('.player-status')?.getAttribute('data-state') ?? null,
          hasDisconnected: !!document.querySelector('.player-disconnected'),
          noFramesText: document.querySelector('.player-disconnected .player-center-title')?.textContent?.trim() ?? null,
          smVideoTrack: window.__smVideoTrack === true,
          smPcState: window.__smPc?.connectionState ?? null,
          smPcIce: window.__smPc?.iceConnectionState ?? null,
          receiverStats: await (async () => {
            const pc = window.__smPc;
            if (!pc) return null;
            const reports = await pc.getStats();
            for (const report of reports.values()) {
              if (report.type === 'inbound-rtp' && report.kind === 'video') {
                return {
                  framesDecoded: report.framesDecoded ?? 0,
                  framesReceived: report.framesReceived ?? 0,
                  jitterBufferDelay: report.jitterBufferDelay ?? 0,
                  jitterBufferEmittedCount: report.jitterBufferEmittedCount ?? 0,
                  packetsLost: report.packetsLost ?? 0,
                };
              }
            }
            return null;
          })(),
        };
        if (v && v.videoWidth > 0 && v.videoHeight > 0) {
          try {
            const c = document.createElement('canvas');
            c.width = v.videoWidth;
            c.height = v.videoHeight;
            const ctx = c.getContext('2d');
            ctx.drawImage(v, 0, 0, c.width, c.height);
            const img = ctx.getImageData(0, 0, c.width, c.height).data;
            let mr = 0, mg = 0, mb = 0, nonBlack = 0;
            let signature = 0;
            for (let i = 0; i < img.length; i += 16) {
              const r = img[i], g = img[i + 1], b = img[i + 2];
              signature = (signature + r * 3 + g * 5 + b * 7) >>> 0;
              if (r > mr) mr = r;
              if (g > mg) mg = g;
              if (b > mb) mb = b;
              if (r > 16 || g > 16 || b > 16) nonBlack++;
            }
            snap.pixelStats = { maxR: mr, maxG: mg, maxB: mb, nonBlack, signature };
          } catch (e) {
            snap.pixelStats = { error: e.message };
          }
        }
        return snap;
      });
      trace.push({ tMs: Date.now() - t0, ...snap });
      const ev = [];
      if (snap.smVideoTrack) ev.push('stream-rcvd');
      if (snap.videoWidth > 0) ev.push(`w=${snap.videoWidth}x${snap.videoHeight}`);
      if (snap.readyState >= 1) ev.push(`rs=${snap.readyState}`);
      if (snap.pixelStats && !snap.pixelStats.error &&
          (snap.pixelStats.maxR > 32 || snap.pixelStats.maxG > 32 || snap.pixelStats.maxB > 32)) {
        ev.push(`pixR=${snap.pixelStats.maxR}G=${snap.pixelStats.maxG}B=${snap.pixelStats.maxB}n=${snap.pixelStats.nonBlack}`);
        if (!pixelSample) pixelSample = { tMs: Date.now() - t0, ...snap.pixelStats };
      }
      if (snap.pixelStats && baseline && !firstChangedAt && snap.pixelStats.nonBlack > 1000 && snap.pixelStats.signature !== baseline.signature) {
        firstChangedAt = Date.now() - t0;
        const hostShot = '/tmp/clock-stopwatch-host-at-viewer-change.png';
        spawnSync('screencapture', ['-x', hostShot]);
        await page.screenshot({ path: path.join(OUT_DIR, 'viewer-clock-at-change.png'), fullPage: true });
        const canvasShot = await page.evaluate(() => {
          const v = document.querySelector('video.frame');
          if (!v || !v.videoWidth) return null;
          const c = document.createElement('canvas');
          c.width = v.videoWidth;
          c.height = v.videoHeight;
          const ctx = c.getContext('2d');
          ctx.drawImage(v, 0, 0, c.width, c.height);
          return c.toDataURL('image/png');
        });
        if (canvasShot) {
          fs.writeFileSync(path.join(OUT_DIR, 'viewer-clock-decoded-frame.png'), Buffer.from(canvasShot.split(',')[1], 'base64'));
        }
        console.log(`[diag] first changed decoded frame at +${firstChangedAt}ms; latency=${firstChangedAt - changeTriggeredAt}ms; hostShot=${hostShot}; viewerShot=${path.join(OUT_DIR, 'viewer-clock-at-change.png')}; decodedFrame=${path.join(OUT_DIR, 'viewer-clock-decoded-frame.png')}`);
      }
      if (snap.statusPill) ev.push(`pill=${snap.statusPill}`);
      if (snap.hasDisconnected) ev.push(`disconnect("${snap.noFramesText}")`);
      console.log(`[diag] +${Date.now() - t0}ms ${ev.join(' ') || '(no events)'}`);
      await new Promise((r) => setTimeout(r, 200));
    }

    fs.writeFileSync(path.join(OUT_DIR, 'diag-direct-trace.json'), JSON.stringify(trace, null, 2));
    await page.screenshot({ path: path.join(OUT_DIR, 'diag-direct.png'), fullPage: true });

    const final = trace[trace.length - 1];
    console.log('\n=== FINAL STATE ===');
    console.log(JSON.stringify(final, null, 2));

    await browser.disconnect();
  } catch (e) {
    console.error('[diag] FAIL:', e.message);
    console.error(e.stack);
  } finally {
    killChrome();
    killTauri();
    setTimeout(() => process.exit(0), 200);
  }
}

main();
