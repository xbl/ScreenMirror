#!/usr/bin/env node
/**
 * Build or validate the generated ScreenMirror icon assets.
 */
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { fileURLToPath, pathToFileURL } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const ICON_DIR = path.join(ROOT, 'src-tauri', 'icons');
const HTML = path.join(__dirname, 'build-icon.html');
const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const ICNS_SIGNATURE = Buffer.from('icns', 'ascii');
const ICO_SIGNATURE = Buffer.from([0x00, 0x00, 0x01, 0x00]);
const ICNS_OUTPUT = 'icon.icns';
const ICO_OUTPUT = 'icon.ico';
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

// --- Dependency-free PNG RGBA decoder ----------------------------------------
// Parses the PNG chunks in order, inflates the concatenated IDAT payload with
// node:zlib, and reverses the per-scanline PNG filters (types 0..4). The
// decoder intentionally accepts only color type 6 (RGBA), 8-bit channels, no
// interlace: that is the only output Chrome/Puppeteer produces for our
// screenshot pipeline, and rejecting anything else keeps the contract honest.

function readChunk(buffer, offset) {
  // The chunk header is 8 bytes (length + type), data is `length` bytes, and
  // a 4-byte CRC follows. Validate everything up front so a truncated PNG
  // reports a clear PNG-specific failure rather than a generic RangeError.
  if (offset + 8 > buffer.length) {
    throw new Error(`PNG chunk header truncated at offset ${offset}`);
  }
  const length = buffer.readUInt32BE(offset);
  const type = buffer.toString('ascii', offset + 4, offset + 8);
  const dataStart = offset + 8;
  const dataEnd = dataStart + length;
  if (dataEnd + 4 > buffer.length) {
    throw new Error(
      `PNG chunk ${type} truncated: declared length ${length} exceeds remaining bytes`,
    );
  }
  const data = buffer.subarray(dataStart, dataEnd);
  // CRC covers type + data; we verify to reject malformed PNGs early instead
  // of trusting the byte stream.
  const expectedCrc = buffer.readUInt32BE(dataEnd);
  const crc = crc32(Buffer.concat([Buffer.from(type, 'ascii'), data]));
  if (crc !== expectedCrc) {
    throw new Error(`PNG chunk ${type} failed CRC check`);
  }
  return { type, data, nextOffset: dataEnd + 4 };
}

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(buffer) {
  let c = 0xffffffff;
  for (let i = 0; i < buffer.length; i += 1) {
    c = CRC_TABLE[(c ^ buffer[i]) & 0xff] ^ (c >>> 8);
  }
  return (c ^ 0xffffffff) >>> 0;
}

function paethPredictor(a, b, c) {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  if (pa <= pb && pa <= pc) return a;
  if (pb <= pc) return b;
  return c;
}

export function decodeRgbaPng(buffer) {
  if (!Buffer.isBuffer(buffer)) {
    throw new Error('decodeRgbaPng expects a Buffer');
  }
  if (!buffer.subarray(0, 8).equals(PNG_SIGNATURE)) {
    throw new Error('invalid PNG signature');
  }

  let offset = 8;
  let width = 0;
  let height = 0;
  let bitDepth = 0;
  let colorType = 0;
  let interlace = 0;
  const idatChunks = [];

  while (offset < buffer.length) {
    const { type, data, nextOffset } = readChunk(buffer, offset);
    if (type === 'IHDR') {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      bitDepth = data[8];
      colorType = data[9];
      interlace = data[12];
    } else if (type === 'IDAT') {
      idatChunks.push(Buffer.from(data));
    } else if (type === 'IEND') {
      offset = nextOffset;
      break;
    }
    offset = nextOffset;
  }

  if (width === 0 || height === 0) {
    throw new Error('PNG missing IHDR');
  }
  if (bitDepth !== 8) {
    throw new Error(`PNG bit depth ${bitDepth} not supported (expected 8)`);
  }
  if (colorType !== 6) {
    throw new Error(`PNG color type ${colorType} not supported (expected 6/RGBA)`);
  }
  if (interlace !== 0) {
    throw new Error('interlaced PNGs are not supported');
  }

  const compressed = Buffer.concat(idatChunks);
  const inflated = zlib.inflateSync(compressed);
  const stride = width * 4;
  const expected = height * (stride + 1);
  if (inflated.length !== expected) {
    throw new Error(`inflated PNG payload length ${inflated.length} != expected ${expected}`);
  }

  const rgba = Buffer.alloc(width * height * 4);
  const prev = new Uint8Array(stride);
  let rowStart = 0;
  for (let y = 0; y < height; y += 1) {
    const filter = inflated[rowStart];
    const row = inflated.subarray(rowStart + 1, rowStart + 1 + stride);
    const out = new Uint8Array(stride);
    for (let x = 0; x < stride; x += 1) {
      const cur = row[x];
      const left = x >= 4 ? out[x - 4] : 0;
      const up = prev[x];
      const upLeft = x >= 4 ? prev[x - 4] : 0;
      let recon;
      switch (filter) {
        case 0:
          recon = cur;
          break;
        case 1:
          recon = (cur + left) & 0xff;
          break;
        case 2:
          recon = (cur + up) & 0xff;
          break;
        case 3:
          recon = (cur + ((left + up) >> 1)) & 0xff;
          break;
        case 4:
          recon = (cur + paethPredictor(left, up, upLeft)) & 0xff;
          break;
        default:
          throw new Error(`unknown PNG filter ${filter}`);
      }
      out[x] = recon;
    }
    rgba.set(out, y * stride);
    prev.set(out);
    rowStart += stride + 1;
  }

  return { width, height, rgba };
}

// --- Visual contract validators ----------------------------------------------
// Pure helpers exported for direct programmatic use. Throwing a specific Error
// makes the contract failure attributable to one violation, which keeps
// debugging straightforward when --check fails during regeneration.

const APP_PALETTE = {
  black: { r: 0, g: 0, b: 0 },
  white: { r: 255, g: 255, b: 255 },
  orange: { r: 255, g: 138, b: 36 },
};
const TRAY_EXPECTED_SIZE = 44;
const TRAY_COLLAR_PIXEL_DELTA = 12;

function cornerAlphaAt({ width, rgba }, x, y) {
  return rgba[(y * width + x) * 4 + 3];
}

export function validateAppIconPixels({ width, height, rgba }) {
  if (!Buffer.isBuffer(rgba) || rgba.length !== width * height * 4) {
    throw new Error('app icon rgba buffer has the wrong length');
  }
  const corners = [
    cornerAlphaAt({ width, rgba }, 0, 0),
    cornerAlphaAt({ width, rgba }, width - 1, 0),
    cornerAlphaAt({ width, rgba }, 0, height - 1),
    cornerAlphaAt({ width, rgba }, width - 1, height - 1),
  ];
  if (!corners.every((alpha) => alpha === 0)) {
    throw new Error(`app icon corners must be fully transparent, got [${corners.join(', ')}]`);
  }

  let hasOpaqueBlack = false;
  let hasOpaqueWhite = false;
  let hasOrange = false;
  for (let i = 0; i < rgba.length; i += 4) {
    const r = rgba[i];
    const g = rgba[i + 1];
    const b = rgba[i + 2];
    const a = rgba[i + 3];
    if (a === 255 && r === 0 && g === 0 && b === 0) hasOpaqueBlack = true;
    if (a === 255 && r === 255 && g === 255 && b === 255) hasOpaqueWhite = true;
    if (
      a === 255 &&
      r === APP_PALETTE.orange.r &&
      g === APP_PALETTE.orange.g &&
      b === APP_PALETTE.orange.b
    ) {
      hasOrange = true;
    }
  }

  if (!hasOpaqueBlack) {
    throw new Error('app icon must contain an opaque pure-black pixel');
  }
  if (!hasOpaqueWhite) {
    throw new Error('app icon must contain an opaque pure-white pixel');
  }
  if (!hasOrange) {
    throw new Error('app icon must contain an opaque exact #FF8A24 orange pixel');
  }
}

export function validateTrayPixels(disconnected, connected) {
  for (const [label, image] of [
    ['disconnected', disconnected],
    ['connected', connected],
  ]) {
    if (!image || image.width !== TRAY_EXPECTED_SIZE || image.height !== TRAY_EXPECTED_SIZE) {
      throw new Error(
        `${label} tray icon must be ${TRAY_EXPECTED_SIZE}x${TRAY_EXPECTED_SIZE}, got ` +
          `${image?.width}x${image?.height}`,
      );
    }
    if (!Buffer.isBuffer(image.rgba) || image.rgba.length !== TRAY_EXPECTED_SIZE ** 2 * 4) {
      throw new Error(`${label} tray icon rgba buffer has the wrong length`);
    }
    const corners = [
      cornerAlphaAt(image, 0, 0),
      cornerAlphaAt(image, TRAY_EXPECTED_SIZE - 1, 0),
      cornerAlphaAt(image, 0, TRAY_EXPECTED_SIZE - 1),
      cornerAlphaAt(image, TRAY_EXPECTED_SIZE - 1, TRAY_EXPECTED_SIZE - 1),
    ];
    if (!corners.every((alpha) => alpha === 0)) {
      throw new Error(`${label} tray icon corners must be fully transparent`);
    }
    let visiblePixels = 0;
    for (let i = 0; i < image.rgba.length; i += 4) {
      const a = image.rgba[i + 3];
      if (a === 0) continue;
      visiblePixels += 1;
      const r = image.rgba[i];
      const g = image.rgba[i + 1];
      const b = image.rgba[i + 2];
      // Template Image pixels must read as a single channel; allow antialiased
      // gray (not strictly black) so the macOS menubar can recolor freely.
      if (r !== g || g !== b) {
        throw new Error(`${label} tray icon visible pixel at byte ${i} is not grayscale`);
      }
    }
    if (visiblePixels === 0) {
      throw new Error(`${label} tray icon must contain visible pixels`);
    }
  }

  if (disconnected.rgba.equals(connected.rgba)) {
    throw new Error('connected and disconnected tray icons must not be byte-identical');
  }

  // Count visible pixels in each state, then subtract the disconnected set
  // from the connected set. We require at least 12 new visible pixels so a
  // collar survives the 22-point menubar scaling.
  const disconnectedVisible = new Set();
  for (let i = 0; i < disconnected.rgba.length; i += 4) {
    if (disconnected.rgba[i + 3] > 0) {
      disconnectedVisible.add(
        `${disconnected.rgba[i]},${disconnected.rgba[i + 1]},${disconnected.rgba[i + 2]},${disconnected.rgba[i + 3]}|${i}`,
      );
    }
  }
  let newVisible = 0;
  for (let i = 0; i < connected.rgba.length; i += 4) {
    if (connected.rgba[i + 3] === 0) continue;
    const key = `${connected.rgba[i]},${connected.rgba[i + 1]},${connected.rgba[i + 2]},${connected.rgba[i + 3]}|${i}`;
    if (!disconnectedVisible.has(key)) newVisible += 1;
  }
  if (newVisible < TRAY_COLLAR_PIXEL_DELTA) {
    throw new Error(
      `connected tray icon must add at least ${TRAY_COLLAR_PIXEL_DELTA} visible pixels ` +
        `compared to disconnected, got ${newVisible}`,
    );
  }
}

function checkVisualContracts() {
  const app = decodeRgbaPng(fs.readFileSync(path.join(ICON_DIR, 'icon.png')));
  validateAppIconPixels(app);

  const disconnected = decodeRgbaPng(fs.readFileSync(path.join(ICON_DIR, 'tray-disconnected.png')));
  const connected = decodeRgbaPng(fs.readFileSync(path.join(ICON_DIR, 'tray-connected.png')));
  validateTrayPixels(disconnected, connected);

  console.log('OK visual contracts: app palette and tray states');
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

  for (const { name, signature } of [
    { name: ICNS_OUTPUT, signature: ICNS_SIGNATURE },
    { name: ICO_OUTPUT, signature: ICO_SIGNATURE },
  ]) {
    const outputPath = path.join(ICON_DIR, name);
    try {
      const buffer = fs.readFileSync(outputPath);
      if (!buffer.subarray(0, signature.length).equals(signature)) {
        console.error(`FAIL ${name}: missing ${signature.toString('hex')} magic header`);
        valid = false;
        continue;
      }
      console.log(`OK ${name}: magic header ${signature.toString('hex')}`);
    } catch (error) {
      console.error(`FAIL ${name}: ${error.message}`);
      valid = false;
    }
  }

  if (valid) {
    try {
      checkVisualContracts();
    } catch (error) {
      console.error(`FAIL visual contracts: ${error.message}`);
      valid = false;
    }
  } else {
    console.error('SKIP visual contracts: structural checks failed');
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

  // After PNG generation, delegate to the project-local Tauri CLI to produce
  // the platform icon assets (icon.icns, icon.ico, etc.) from the canonical
  // icon.png. This keeps a single source of truth and avoids the bundler
  // failing with `No matching IconType` from a 1024×1024 non-retina source.
  await generatePlatformIcons();
}

function findTauriCli() {
  const localBin = path.join(
    ROOT,
    'node_modules',
    '.bin',
    process.platform === 'win32' ? 'tauri.cmd' : 'tauri',
  );
  if (fs.existsSync(localBin)) {
    return localBin;
  }
  throw new Error('Could not find the Tauri CLI binary. Did you forget to run `npm install`?');
}

async function generatePlatformIcons() {
  const { spawn } = await import('node:child_process');
  const tauriCli = findTauriCli();
  const sourceIcon = path.join(ICON_DIR, 'icon.png');
  const { execPath } = process;
  // The Tauri CLI is a NAPI binary; run it via the local Node binary so the
  // icon generation stays reproducible from `npm run icons`.
  const child = spawn(execPath, [tauriCli, 'icon', sourceIcon, '--output', ICON_DIR], {
    stdio: 'inherit',
  });
  const exitCode = await new Promise((resolve, reject) => {
    child.on('error', reject);
    child.on('exit', (code) => resolve(code ?? 1));
  });
  if (exitCode !== 0) {
    throw new Error(`tauri icon exited with code ${exitCode}`);
  }
}

// CLI entry point: only run when this file is invoked directly. Importing the
// module (for example from a test or one-off script) must not launch
// Puppeteer, spawn the Tauri CLI, or otherwise trigger generation side
// effects.
const isMainEntry = (() => {
  if (!process.argv[1]) return false;
  return import.meta.url === pathToFileURL(process.argv[1]).href;
})();

if (isMainEntry) {
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
}
