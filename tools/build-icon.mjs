#!/usr/bin/env node
/**
 * Build or validate the generated Screenmirror icon assets.
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const ICON_DIR = path.join(ROOT, 'src-tauri', 'icons');
const HTML = path.join(__dirname, 'build-icon.html');
const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const VIEWS = ['app-icon', 'tray-disconnected', 'tray-connected'];

export const OUTPUTS = [
  { name: 'icon.png', view: 'app-icon', size: 1024 },
  { name: 'icon-512.png', view: 'app-icon', size: 512 },
  { name: '128x128@2x.png', view: 'app-icon', size: 256 },
  { name: '128x128.png', view: 'app-icon', size: 128 },
  { name: '32x32.png', view: 'app-icon', size: 32 },
  { name: 'tray-disconnected.png', view: 'tray-disconnected', size: 44 },
  { name: 'tray-connected.png', view: 'tray-connected', size: 44 },
];

export function readPngDimensions(buffer) {
  if (!buffer.subarray(0, 8).equals(PNG_SIGNATURE)) {
    throw new Error('invalid PNG signature');
  }
  if (buffer.toString('ascii', 12, 16) !== 'IHDR') {
    throw new Error('missing PNG IHDR');
  }
  return {
    width: buffer.readUInt32BE(16),
    height: buffer.readUInt32BE(20),
  };
}

function checkOutputs() {
  let valid = true;

  for (const { name, size } of OUTPUTS) {
    const outputPath = path.join(ICON_DIR, name);
    try {
      const dimensions = readPngDimensions(fs.readFileSync(outputPath));
      if (dimensions.width !== size || dimensions.height !== size) {
        console.error(
          `FAIL ${name}: expected ${size}x${size}, got ${dimensions.width}x${dimensions.height}`,
        );
        valid = false;
        continue;
      }
      console.log(`OK ${name}: ${dimensions.width}x${dimensions.height}`);
    } catch (error) {
      console.error(`FAIL ${name}: ${error.message}`);
      valid = false;
    }
  }

  if (!valid) {
    process.exitCode = 1;
  }
}

const MACOS_CHROME_CANDIDATES = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary',
  '/Applications/Chromium.app/Contents/MacOS/Chromium',
];

function resolveChromeExecutable() {
  const fromEnv = process.env.PUPPETEER_EXECUTABLE_PATH;
  const candidates = [fromEnv, ...MACOS_CHROME_CANDIDATES].filter(Boolean);
  const tried = [];
  // Track the most specific reason we rejected the PUPPETEER_EXECUTABLE_PATH
  // entry so we can surface a precise diagnostic instead of letting Puppeteer
  // fail later with a less helpful "Failed to launch the browser process".
  let envRejection = null;
  for (const candidate of candidates) {
    tried.push(candidate);
    let stat;
    try {
      stat = fs.statSync(candidate);
    } catch {
      // missing or not accessible; try the next candidate
      continue;
    }
    if (!stat.isFile()) {
      if (candidate === fromEnv && envRejection === null) {
        envRejection = 'it is not a regular file (point at a binary, not a directory)';
      }
      continue;
    }
    // On POSIX, a regular file is only runnable by Puppeteer when at least one
    // of the execute bits is set. Reject obvious mismatches up front so the
    // error message names the path instead of bubbling out of puppeteer.launch().
    if (process.platform !== 'win32') {
      try {
        fs.accessSync(candidate, fs.constants.X_OK);
      } catch {
        if (candidate === fromEnv && envRejection === null) {
          envRejection =
            'it is not executable (missing the execute bit; run `chmod +x ' +
            candidate +
            '` or pick a different binary)';
        }
        continue;
      }
    }
    return candidate;
  }
  const envHint = fromEnv
    ? `\nThe PUPPETEER_EXECUTABLE_PATH value '${fromEnv}' was rejected because ${envRejection ?? 'it is not a usable Chrome/Chromium binary'}.\n` +
      'Unset PUPPETEER_EXECUTABLE_PATH or point it at an executable Chrome/Chromium binary.'
    : '';
  throw new Error(
    'Could not find a Chrome/Chromium executable to drive Puppeteer.\n' +
      'Tried the following paths:\n' +
      tried.map((p) => `  - ${p}`).join('\n') +
      envHint +
      '\nSet the PUPPETEER_EXECUTABLE_PATH environment variable to an absolute ' +
      'path of an executable Chrome/Chromium binary (e.g. /Applications/Google Chrome.app/Contents/MacOS/Google Chrome).',
  );
}

async function generateOutputs() {
  // Resolve the Chrome/Chromium binary first so a missing executable produces
  // a clear error before we pay the cost of loading puppeteer-core.
  const executablePath = resolveChromeExecutable();
  // puppeteer-core is only needed when we actually render; load it lazily so
  // `--check` stays free of side effects from a heavyweight native module.
  const { default: puppeteer } = await import('puppeteer-core');
  fs.mkdirSync(ICON_DIR, { recursive: true });
  const browser = await puppeteer.launch({
    executablePath,
    headless: true,
    args: ['--no-sandbox', '--disable-dev-shm-usage', '--hide-scrollbars'],
  });
  try {
    for (const { name, size, view } of OUTPUTS) {
      const page = await browser.newPage();
      await page.setViewport({ width: size, height: size, deviceScaleFactor: 1 });
      await page.goto('file://' + HTML, { waitUntil: 'networkidle0' });
      await page.evaluate(
        (targetSize, targetView, allViews) => {
          const svg = document.getElementById('icon-source');
          svg.setAttribute('width', String(targetSize));
          svg.setAttribute('height', String(targetSize));
          svg.setAttribute('viewBox', '0 0 1024 1024');
          for (const id of allViews) {
            const node = document.getElementById(id);
            if (node) {
              node.style.display = id === targetView ? '' : 'none';
            }
          }
          document.body.style.margin = '0';
          document.body.style.background = 'transparent';
        },
        size,
        view,
        VIEWS,
      );
      const buffer = await page.screenshot({
        type: 'png',
        omitBackground: true,
        clip: { x: 0, y: 0, width: size, height: size },
      });
      const outputPath = path.join(ICON_DIR, name);
      fs.writeFileSync(outputPath, buffer);
      console.log('wrote', outputPath, buffer.length, 'bytes');
      await page.close();
    }
  } finally {
    await browser.close();
  }
}

const args = process.argv.slice(2);
if (args.length > 1 || (args.length === 1 && args[0] !== '--check')) {
  console.error(`Unknown arguments: ${args.join(' ') || '(none)'}`);
  console.error('Usage: node tools/build-icon.mjs [--check]');
  process.exitCode = 1;
} else if (args[0] === '--check') {
  checkOutputs();
} else {
  generateOutputs().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
