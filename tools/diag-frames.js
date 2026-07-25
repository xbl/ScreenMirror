#!/usr/bin/env node
/**
 * Diagnostic: spawn Tauri host, connect viewer, deeply probe the <video> element
 * and MediaStream to figure out why videoWidth stays 0.
 */
import { spawn } from 'node:child_process';
import path from 'node:path';
import http from 'node:http';
import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const TAURI_BIN = path.join(ROOT, 'src-tauri', 'target', 'debug', 'screenmirror');
const VIEWER_DIST = path.join(ROOT, 'viewer', 'dist');
const OUT_DIR = path.join(__dirname, 'output');
const ROOM_ID = 'diag-frames';
const PORT = 3131;
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
  throw new Error('server did not start');
}

async function main() {
  fs.mkdirSync(OUT_DIR, { recursive: true });
  console.log('[diag] launching Tauri binary...');
  const proc = spawn(TAURI_BIN, [], {
    env: {
      ...process.env,
      RUST_LOG: 'info',
      SCREENMIRROR_PORT: String(PORT),
      SCREENMIRROR_TEST_ROOM: ROOM_ID,
      SCREENMIRROR_CAPTURE: 'screen',
      SCREENMIRROR_E2E_AUTO_APPROVE: '1',
      SCREENMIRROR_MAX_DIM: '640',
      SCREENMIRROR_CAPTURE_QUALITY: '0.5',
      VIEWER_DIST,
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
    console.log('[diag] server up');

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
      console.log(`[viewer:${msg.type()}]`, msg.text());
    });
    page.on('pageerror', (err) => console.log('[viewer-error]', err.message));

    await page.goto(`${BASE}/${ROOM_ID}`, { waitUntil: 'networkidle2', timeout: 30000 });
    console.log('[diag] page loaded');

    // Probe at multiple time points
    const probePoints = [1000, 2500, 4000, 6000, 8000, 11000, 14000];
    for (const ms of probePoints) {
      await new Promise((r) => setTimeout(r, ms === probePoints[0] ? ms : ms - probePoints[probePoints.indexOf(ms) - 1]));
      const probe = await page.evaluate(() => {
        const v = document.querySelector('video.frame');
        const stream = v?.srcObject;
        const tracks = stream instanceof MediaStream ? stream.getTracks().map(t => ({
          kind: t.kind,
          id: t.id,
          label: t.label,
          muted: t.muted,
          enabled: t.enabled,
          readyState: t.readyState,
        })) : null;
        const pc = window.__smPc;
        return {
          videoEl: !!v,
          videoWidth: v?.videoWidth ?? -1,
          videoHeight: v?.videoHeight ?? -1,
          readyState: v?.readyState ?? -1,
          networkState: v?.networkState ?? -1,
          error: v?.error ? { code: v.error.code, message: v.error.message } : null,
          currentTime: v?.currentTime ?? -1,
          paused: v?.paused ?? null,
          srcObjectIsStream: stream instanceof MediaStream,
          srcObjectTracks: tracks,
          trackCount: tracks?.length ?? 0,
          pcState: pc?.connectionState ?? null,
          iceState: pc?.iceConnectionState ?? null,
          statusPill: document.querySelector('.player-status')?.getAttribute('data-state') ?? null,
          statusText: document.querySelector('.player-status')?.textContent?.trim() ?? null,
          noFramesCard: !!document.querySelector('.player-disconnected .player-center-title'),
          noFramesText: document.querySelector('.player-disconnected .player-center-title')?.textContent ?? null,
        };
      });
      console.log(`[diag t=${ms}ms]`, JSON.stringify(probe, null, 2));
    }

    // Check what video frames are encoded in host
    const shot = path.join(OUT_DIR, 'diag-frames.png');
    await page.screenshot({ path: shot, fullPage: true });
    console.log('[diag] screenshot:', shot);

    await browser.close();
    cleanup();
  } catch (e) {
    console.error('[diag] FAIL:', e.message);
    console.error(e.stack);
    cleanup();
    process.exit(1);
  }
}

main();
