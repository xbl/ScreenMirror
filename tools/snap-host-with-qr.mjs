import puppeteer from 'puppeteer-core';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUT = path.join(__dirname, 'output', 'host-with-qr.png');

// Tiny mock server that serves dist/ statically and answers /api/host-info
// with a fake LAN IP + port. Lets us screenshot the QR card in its "ready"
// state without launching the full Tauri runtime.
const distRoot = path.resolve(__dirname, '..', 'dist');
const mime = (p) => {
  if (p.endsWith('.html')) return 'text/html';
  if (p.endsWith('.js')) return 'application/javascript';
  if (p.endsWith('.css')) return 'text/css';
  if (p.endsWith('.png')) return 'image/png';
  if (p.endsWith('.svg')) return 'image/svg+xml';
  return 'application/octet-stream';
};
const srv = http.createServer(async (req, res) => {
  if (req.url === '/api/host-info') {
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end(
      JSON.stringify({
        lan_ip: '192.168.1.42',
        port: 3131,
        pending_device: null,
        connected_devices: [],
      }),
    );
    return;
  }
  const urlPath = req.url.split('?')[0];
  let filePath = path.join(distRoot, urlPath === '/' ? 'index.html' : urlPath);
  try {
    const fs = await import('node:fs/promises');
    const bytes = await fs.readFile(filePath);
    res.writeHead(200, { 'content-type': mime(filePath) });
    res.end(bytes);
  } catch {
    // SPA fallback
    try {
      const fs = await import('node:fs/promises');
      const bytes = await fs.readFile(path.join(distRoot, 'index.html'));
      res.writeHead(200, { 'content-type': 'text/html' });
      res.end(bytes);
    } catch {
      res.writeHead(404);
      res.end('not found');
    }
  }
});
srv.listen(5181);
await new Promise((r) => setTimeout(r, 400));

try {
  const b = await puppeteer.launch({
    executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    headless: true,
    defaultViewport: { width: 940, height: 720 },
    args: ['--no-sandbox', '--disable-dev-shm-usage'],
  });
  const p = await b.newPage();
  await p.goto('http://127.0.0.1:5181/', { waitUntil: 'networkidle2', timeout: 15000 });
  await new Promise((r) => setTimeout(r, 3000));
  await p.screenshot({ path: OUT, fullPage: false });
  const probe = await p.evaluate(() => {
    const url = document.querySelector('.qr-url')?.textContent?.trim();
    const img = document.querySelector('img.qr-img');
    const copy = document.querySelector('.btn-accent')?.textContent?.trim();
    return { url, qrImg: !!img, qrSrc: img?.src?.slice(0, 40), copy };
  });
  console.log('host-with-qr probe:', JSON.stringify(probe, null, 2));
  await b.close();
  console.log('saved', OUT);
} finally {
  srv.close();
}