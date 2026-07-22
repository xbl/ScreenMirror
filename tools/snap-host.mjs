import puppeteer from 'puppeteer-core';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUT = path.join(__dirname, 'output', 'host-comprehensive.png');

const srv = spawn('python3', ['-m', 'http.server', '5180'], {
  cwd: path.resolve(__dirname, '..', 'dist'),
  stdio: 'ignore',
});
await new Promise((r) => setTimeout(r, 800));

try {
  const b = await puppeteer.launch({
    executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    headless: true,
    defaultViewport: { width: 940, height: 640 },
    args: ['--no-sandbox', '--disable-dev-shm-usage'],
  });
  const p = await b.newPage();
  await p.goto('http://127.0.0.1:5180/', { waitUntil: 'networkidle2', timeout: 15000 });
  await new Promise((r) => setTimeout(r, 2500));
  await p.screenshot({ path: OUT, fullPage: false });
  const probe = await p.evaluate(() => {
    const body = document.body;
    return {
      bg: getComputedStyle(body).backgroundColor,
      text: body.innerText.slice(0, 400),
    };
  });
  console.log('host UI probe:', JSON.stringify(probe, null, 2));
  await b.close();
  console.log('saved', OUT);
} finally {
  srv.kill('SIGTERM');
}