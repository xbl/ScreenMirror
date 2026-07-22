import puppeteer from 'puppeteer-core';
import http from 'node:http';
import path from 'node:path';
import fs from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const distRoot = path.resolve(__dirname, '..', 'dist');
const outPath = path.join(__dirname, 'output', 'host-now.png');

const srv = http.createServer(async (req, res) => {
  if (req.url === '/api/host-info') {
    http.get('http://127.0.0.1:3131/api/host-info', (r) => {
      let b = '';
      r.on('data', (c) => (b += c));
      r.on('end', () => {
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(b);
      });
    });
    return;
  }
  const urlPath = (req.url || '/').split('?')[0];
  const filePath = path.join(distRoot, urlPath === '/' ? 'index.html' : urlPath);
  try {
    const bytes = await fs.readFile(filePath);
    const mime = filePath.endsWith('.html')
      ? 'text/html'
      : filePath.endsWith('.js')
        ? 'application/javascript'
        : filePath.endsWith('.css')
          ? 'text/css'
          : 'application/octet-stream';
    res.writeHead(200, { 'content-type': mime });
    res.end(bytes);
  } catch {
    try {
      const bytes = await fs.readFile(path.join(distRoot, 'index.html'));
      res.writeHead(200, { 'content-type': 'text/html' });
      res.end(bytes);
    } catch {
      res.writeHead(404);
      res.end('not found');
    }
  }
});
srv.listen(5182);
await new Promise((r) => setTimeout(r, 400));
try {
  const b = await puppeteer.launch({
    executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    headless: true,
    defaultViewport: { width: 940, height: 720 },
    args: ['--no-sandbox', '--disable-dev-shm-usage'],
  });
  const p = await b.newPage();
  p.on('console', (m) => console.log('[browser]', m.type(), m.text()));
  p.on('pageerror', (e) => console.log('[browser-error]', e.message));
  p.on('requestfailed', (r) =>
    console.log('[req-failed]', r.url(), r.failure()?.errorText),
  );
  await p.goto('http://127.0.0.1:5182/', { waitUntil: 'networkidle2', timeout: 15000 });
  await new Promise((r) => setTimeout(r, 4000));
  const probe = await p.evaluate(async () => {
    const url = document.querySelector('.qr-url')?.textContent?.trim();
    const empty = !!document.querySelector('.qr-empty');
    const img = document.querySelector('img.qr-img');
    const status = document.querySelector('.qr-status-text')?.textContent?.trim();
    let directFetch = null;
    try {
      const r = await fetch('/api/host-info');
      directFetch = { status: r.status, body: await r.text() };
    } catch (e) {
      directFetch = { error: String(e) };
    }
    return { url, empty, hasImg: !!img, status, directFetch };
  });
  console.log('PROBE:', JSON.stringify(probe, null, 2));
  await p.screenshot({ path: outPath });
  await b.close();
  console.log('saved', outPath);
} finally {
  srv.close();
}