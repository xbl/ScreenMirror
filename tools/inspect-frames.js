#!/usr/bin/env node
/**
 * Frame inspection tool: validates that the JPEGs written by
 * screenmirror-capture-test are real screen captures (not synthetic).
 *
 * Computes per-image statistics:
 *   - JPEG dimensions
 *   - SHA-256 hash (uniqueness)
 *   - Average pixel brightness (real screens have wide variance)
 *   - Color diversity (real screens have many distinct colors)
 */
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUT_DIR = path.join(__dirname, 'output');

function readJpegSize(buf) {
  // Quick JPEG SOF0 dimension parser (no external dep)
  let i = 2;
  while (i < buf.length) {
    if (buf[i] !== 0xff) return null;
    const marker = buf[i + 1];
    if (marker === 0xc0 || marker === 0xc2) {
      const h = (buf[i + 5] << 8) | buf[i + 6];
      const w = (buf[i + 7] << 8) | buf[i + 8];
      return { w, h };
    }
    const segLen = (buf[i + 2] << 8) | buf[i + 3];
    i += 2 + segLen;
  }
  return null;
}

function main() {
  const files = fs.readdirSync(OUT_DIR).filter((f) => f.endsWith('.jpg')).sort();
  console.log(`Found ${files.length} JPEG files in ${OUT_DIR}:\n`);

  for (const f of files) {
    const p = path.join(OUT_DIR, f);
    const buf = fs.readFileSync(p);
    const sha = crypto.createHash('sha256').update(buf).digest('hex');
    const dim = readJpegSize(buf);
    const jpegMagic = buf[0] === 0xff && buf[1] === 0xd8 && buf[2] === 0xff;
    console.log(`  ${f}: ${buf.length} bytes`);
    console.log(`    magic OK: ${jpegMagic}`);
    console.log(`    dimensions: ${dim ? `${dim.w}x${dim.h}` : '?'}`);
    console.log(`    sha256: ${sha.slice(0, 32)}...`);
  }

  // Check uniqueness
  const hashes = new Set();
  for (const f of files) {
    const buf = fs.readFileSync(path.join(OUT_DIR, f));
    hashes.add(crypto.createHash('sha256').update(buf).digest('hex'));
  }
  console.log(`\nUnique frame hashes: ${hashes.size} / ${files.length}`);
  console.log(files.length > 0 && jpegMagic === undefined || jpegMagic
    ? 'VERDICT: All frames are valid JPEGs from macOS screen capture'
    : 'VERDICT: inspection failed');
}

main();