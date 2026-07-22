#!/usr/bin/env node
/**
 * Build the Screenmirror app icon from a hand-authored SVG.
 * Renders the SVG in headless Chrome at multiple sizes and writes PNGs into
 * src-tauri/icons/.
 *
 * Sizes emitted:
 *   icon.png           1024×1024  (canonical)
 *   icon-512.png        512×512
 *   128x128@2x.png      256×256
 *   128x128.png         128×128
 *   32x32.png            32×32
 */
import { spawn } from 'node:child_process';
import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const ICON_DIR = path.join(ROOT, 'src-tauri', 'icons');
const HTML = path.join(__dirname, 'build-icon.html');

const SIZES = [
  { name: 'icon.png', size: 1024 },
  { name: 'icon-512.png', size: 512 },
  { name: '128x128@2x.png', size: 256 },
  { name: '128x128.png', size: 128 },
  { name: '32x32.png', size: 32 },
];

async function main() {
  fs.mkdirSync(ICON_DIR, { recursive: true });
  const browser = await puppeteer.launch({
    executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    headless: true,
    args: ['--no-sandbox', '--disable-dev-shm-usage', '--hide-scrollbars'],
  });
  try {
    for (const { name, size } of SIZES) {
      const page = await browser.newPage();
      await page.setViewport({ width: size, height: size, deviceScaleFactor: 1 });
      await page.goto('file://' + HTML, { waitUntil: 'networkidle0' });
      // Replace the SVG's width/height to the target size and screenshot.
      await page.evaluate((s) => {
        const svg = document.getElementById('icon');
        svg.setAttribute('width', String(s));
        svg.setAttribute('height', String(s));
        document.body.style.margin = '0';
        document.body.style.background = 'transparent';
      }, size);
      const buf = await page.screenshot({
        type: 'png',
        omitBackground: true,
        clip: { x: 0, y: 0, width: size, height: size },
      });
      const out = path.join(ICON_DIR, name);
      fs.writeFileSync(out, buf);
      console.log('wrote', out, buf.length, 'bytes');
      await page.close();
    }
  } finally {
    await browser.close();
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
