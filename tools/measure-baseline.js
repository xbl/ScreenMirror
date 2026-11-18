#!/usr/bin/env node
/**
 * Quantitative benchmark to measure real WebRTC streaming performance:
 * - FPS (frames decoded / second)
 * - Freezes (freezeCount, totalFreezesDuration)
 * - Packet loss (packetsLost)
 * - Jitter and decode time
 * - Frame interval jitter (max frame gap in ms)
 *
 * Runs a moving window on the host during measurement to benchmark motion performance.
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
  ?? path.join(ROOT, 'src-tauri', 'target', 'aarch64-apple-darwin', 'debug', 'screenmirror');
const VIEWER_DIST = path.join(ROOT, 'viewer', 'dist');
const OUT_DIR = path.join(__dirname, 'output');
const ROOM_ID = 'benchmark-room';
const PORT = Number(process.env.BENCH_PORT ?? 3140);
const BASE = `http://127.0.0.1:${PORT}`;
const CHROME_BIN = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const DEBUG_PORT = Number(process.env.BENCH_CHROME_PORT ?? 9450);
const USER_DATA_DIR = `/tmp/screenmirror-bench-${Date.now()}`;
const QUALITY = process.env.BENCH_QUALITY ?? '0.5'; // 0.5 = 1280px, 0.75 = 1920px
const CAPTURE_KIND = process.env.BENCH_CAPTURE ?? 'screen'; // 'screen' or 'test'
const DURATION_SEC = Number(process.env.BENCH_DURATION ?? 8);

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
    await new Promise((r) => setTimeout(r, 200));
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
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error('chrome did not expose debug port');
}

async function main() {
  fs.mkdirSync(OUT_DIR, { recursive: true });
  console.log(`[benchmark] Starting test: quality=${QUALITY}, capture=${CAPTURE_KIND}, duration=${DURATION_SEC}s on port ${PORT}...`);

  const tauri = spawn(TAURI_BIN, ['--port', String(PORT)], {
    env: {
      ...process.env,
      RUST_LOG: 'info',
      SCREENMIRROR_PORT: String(PORT),
      SCREENMIRROR_TEST_ROOM: ROOM_ID,
      SCREENMIRROR_CAPTURE: CAPTURE_KIND,
      SCREENMIRROR_CAPTURE_QUALITY: QUALITY,
      SCREENMIRROR_E2E_AUTO_APPROVE: '1',
      VIEWER_DIST,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  tauri.stderr.on('data', (d) => process.stderr.write('[tauri] ' + d.toString()));

  let cleaned = false;
  const killTauri = () => {
    if (cleaned) return;
    cleaned = true;
    try { tauri.kill('SIGKILL'); } catch {}
  };
  process.on('exit', killTauri);
  process.on('SIGINT', () => { killTauri(); process.exit(130); });

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
    console.log('[benchmark] server ready');

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

    await waitForChrome();
    console.log('[benchmark] chrome connected');

    const browser = await puppeteer.connect({
      browserURL: `http://127.0.0.1:${DEBUG_PORT}`,
      protocolTimeout: 60000,
    });
    const page = await browser.newPage();
    await page.setCacheEnabled(false);

    await page.goto(`${BASE}/${ROOM_ID}`, { waitUntil: 'networkidle2', timeout: 30000 });
    console.log('[benchmark] viewer loaded, waiting for video stream...');

    // Wait until video is streaming
    await page.waitForFunction(() => {
      const v = document.querySelector('video.frame');
      return v && v.videoWidth > 0 && v.readyState >= 2;
    }, { timeout: 15000 });
    console.log('[benchmark] video stream active! Commencing motion test...');

    // Start motion simulation in background if real screen capture
    let motionInterval = null;
    if (CAPTURE_KIND === 'screen') {
      // Launch Calculator and drag it across the screen repeatedly
      spawnSync('osascript', ['-e', 'tell application "Calculator" to activate']);
      let pos = 200;
      let step = 60;
      motionInterval = setInterval(() => {
        pos += step;
        if (pos > 1100 || pos < 200) step = -step;
        spawn('osascript', ['-e', `tell application "System Events" to tell process "Calculator" to set position of window 1 to {${pos}, 250}`]);
      }, 50); // move every 50ms
    }

    // Warm-up 1 second
    await new Promise((r) => setTimeout(r, 1000));

    // Sample stats at 100ms intervals
    const samples = [];
    const startTime = Date.now();
    let prevFramesDecoded = 0;
    let prevTimestamp = startTime;
    const frameGaps = [];

    while (Date.now() - startTime < DURATION_SEC * 1000) {
      const now = Date.now();
      const stats = await page.evaluate(async () => {
        const pc = window.__smPc;
        if (!pc) return null;
        const reports = await pc.getStats();
        for (const report of reports.values()) {
          if (report.type === 'inbound-rtp' && report.kind === 'video') {
            return {
              framesDecoded: report.framesDecoded ?? 0,
              framesReceived: report.framesReceived ?? 0,
              framesDropped: report.framesDropped ?? 0,
              freezeCount: report.freezeCount ?? 0,
              totalFreezesDuration: report.totalFreezesDuration ?? 0,
              packetsLost: report.packetsLost ?? 0,
              packetsReceived: report.packetsReceived ?? 0,
              jitter: report.jitter ?? 0,
              jitterBufferDelay: report.jitterBufferDelay ?? 0,
              jitterBufferEmittedCount: report.jitterBufferEmittedCount ?? 0,
              keyFramesDecoded: report.keyFramesDecoded ?? 0,
              frameWidth: report.frameWidth ?? 0,
              frameHeight: report.frameHeight ?? 0,
              framesPerSecond: report.framesPerSecond ?? 0,
            };
          }
        }
        return null;
      });

      if (stats) {
        if (stats.framesDecoded > prevFramesDecoded) {
          const deltaMs = now - prevTimestamp;
          frameGaps.push(deltaMs);
          prevFramesDecoded = stats.framesDecoded;
          prevTimestamp = now;
        }
        samples.push({ tMs: now - startTime, ...stats });
      }

      await new Promise((r) => setTimeout(r, 100));
    }

    if (motionInterval) {
      clearInterval(motionInterval);
      spawnSync('osascript', ['-e', 'tell application "Calculator" to quit']);
    }

    const first = samples[0];
    const last = samples[samples.length - 1];
    const totalTimeSec = (last.tMs - first.tMs) / 1000;
    const totalFrames = last.framesDecoded - first.framesDecoded;
    const avgFps = totalFrames / totalTimeSec;
    const freezes = last.freezeCount - first.freezeCount;
    const freezeDuration = last.totalFreezesDuration - first.totalFreezesDuration;
    const packetsLost = last.packetsLost - first.packetsLost;
    const keyFrames = last.keyFramesDecoded - first.keyFramesDecoded;
    const maxGapMs = frameGaps.length > 0 ? Math.max(...frameGaps) : 0;
    const avgGapMs = frameGaps.length > 0 ? (frameGaps.reduce((a, b) => a + b, 0) / frameGaps.length) : 0;

    const report = {
      quality: QUALITY,
      captureKind: CAPTURE_KIND,
      resolution: `${last.frameWidth}x${last.frameHeight}`,
      durationSec: totalTimeSec.toFixed(1),
      totalFramesDecoded: totalFrames,
      avgFps: avgFps.toFixed(1),
      freezeCount: freezes,
      totalFreezesDurationSec: freezeDuration.toFixed(3),
      packetsLost: packetsLost,
      keyFramesDecoded: keyFrames,
      avgFrameGapMs: avgGapMs.toFixed(1),
      maxFrameGapMs: maxGapMs.toFixed(1),
      avgJitterMs: ((last.jitter ?? 0) * 1000).toFixed(2),
    };

    console.log('\n================== BENCHMARK REPORT ==================');
    console.table([report]);
    console.log('======================================================\n');

    fs.writeFileSync(path.join(OUT_DIR, `bench-${QUALITY}-${CAPTURE_KIND}.json`), JSON.stringify({ report, samples }, null, 2));

    await browser.disconnect();
    return report;
  } finally {
    killChrome();
    killTauri();
  }
}

main().then(() => process.exit(0)).catch((err) => {
  console.error('[benchmark] Error:', err);
  process.exit(1);
});
