#!/usr/bin/env node
/**
 * Focused verification: the Reconnect button is clickable and triggers a
 * page reload (which would let the viewer retry the WebRTC handshake).
 *
 * Steps:
 *   1. Run the same flow as verify-fix.js to get to noFrames card.
 *   2. Click the Reconnect button.
 *   3. Verify the page navigates (location changes, view remounts).
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
const ROOM_ID = 'verify-reconnect';
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
  console.log('[verify-rec] launching Tauri binary...');
  const proc = spawn(TAURI_BIN, [], {
    env: {
      ...process.env,
      RUST_LOG: 'info',
      SCREENMIRROR_PORT: String(PORT),
      SCREENMIRROR_TEST_ROOM: ROOM_ID,
      SCREENMIRROR_CAPTURE: 'screen',
      SCREENMIRROR_E2E_AUTO_APPROVE: '1',
      SCREENMIRROR_MAX_DIM: '480',
      SCREENMIRROR_CAPTURE_QUALITY: '0.3',
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
    console.log('[verify-rec] server up');

    const browser = await puppeteer.launch({
      executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      headless: true,
      protocolTimeout: 60000,
      args: ['--no-sandbox', '--autoplay-policy=no-user-gesture-required',
        '--allow-running-insecure-content',
        '--unsafely-treat-insecure-origin-as-secure=http://127.0.0.1:3131'],
      defaultViewport: { width: 1280, height: 800 },
    });
    const page = await browser.newPage();
    await page.setCacheEnabled(false);
    let reloadCount = 0;
    page.on('framenavigated', (frame) => {
      if (frame === page.mainFrame()) reloadCount++;
    });
    page.on('console', (msg) => {
      if (/player-view|early-offer|no frames|reconnect/i.test(msg.text())) {
        console.log(`[viewer:${msg.type()}]`, msg.text().slice(0, 300));
      }
    });

    await page.goto(`${BASE}/${ROOM_ID}`, { waitUntil: 'networkidle2', timeout: 30000 });
    const initialNav = reloadCount;
    console.log(`[verify-rec] initial navigation: ${initialNav}`);

    // Wait for noFrames card
    let cardShown = false;
    const t0 = Date.now();
    while (Date.now() - t0 < 15000) {
      const found = await page.evaluate(() => !!document.querySelector('.player-disconnected .player-reconnect'));
      if (found) { cardShown = true; break; }
      await new Promise((r) => setTimeout(r, 250));
    }
    if (!cardShown) {
      console.log('[verify-rec] FAIL: noFrames card never appeared');
      cleanup();
      process.exit(1);
    }
    console.log('[verify-rec] noFrames card visible');

    // Snapshot state before clicking
    const before = await page.evaluate(() => ({
      url: location.href,
      disconnected: !!document.querySelector('.player-disconnected'),
      btnText: document.querySelector('.player-reconnect')?.textContent?.trim() ?? '',
      statusPill: document.querySelector('.player-status')?.getAttribute('data-state') ?? '',
    }));
    console.log('[verify-rec] before click:', JSON.stringify(before));

    // Click the Reconnect button
    const clicked = await page.evaluate(() => {
      const btn = document.querySelector('.player-reconnect');
      if (!btn) return { clicked: false, reason: 'no button' };
      const rect = btn.getBoundingClientRect();
      const visible = rect.width > 0 && rect.height > 0;
      btn.click();
      return { clicked: true, visible, rect: { x: rect.x, y: rect.y, w: rect.width, h: rect.height } };
    });
    console.log('[verify-rec] click result:', JSON.stringify(clicked));

    // Wait for reload
    await new Promise((r) => setTimeout(r, 2000));
    const after = await page.evaluate(() => ({
      url: location.href,
      bodyText: document.body.innerText.slice(0, 200),
    }));
    console.log('[verify-rec] after click + 2s:', JSON.stringify(after));

    const reloaded = reloadCount > initialNav;
    console.log(`[verify-rec] reload observed: ${reloaded} (${initialNav} -> ${reloadCount})`);

    const shot = path.join(OUT_DIR, 'verify-reconnect.png');
    await page.screenshot({ path: shot, fullPage: true });
    console.log(`[verify-rec] screenshot: ${shot}`);

    let pass = reloaded && clicked.clicked && clicked.visible;
    console.log(`\n=== RECONNECT VERIFICATION ===`);
    console.log(`  card visible: ${cardShown}`);
    console.log(`  button found: ${clicked.clicked}`);
    console.log(`  button visible: ${clicked.visible} rect=${JSON.stringify(clicked.rect)}`);
    console.log(`  reload triggered: ${reloaded}`);
    console.log(`  VERDICT: ${pass ? '✅ Reconnect works end-to-end' : '❌ Reconnect broken'}`);

    await browser.close();
    cleanup();
    process.exit(pass ? 0 : 1);
  } catch (e) {
    console.error('[verify-rec] FAIL:', e.message);
    console.error(e.stack);
    cleanup();
    process.exit(1);
  }
}

main();