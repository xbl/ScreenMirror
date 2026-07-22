// Diagnostic: open the dev-mode UI like Tauri does (same origin localhost:5173)
// and observe what QRCard's fetchHostInfo actually does when IPC works.
import puppeteer from 'puppeteer-core';

const browser = await puppeteer.launch({
  executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-setuid-sandbox'],
});
const page = await browser.newPage();

page.on('console', (msg) => {
  console.log(`[browser ${msg.type()}]`, msg.text());
});
page.on('pageerror', (err) => console.log('[browser pageerror]', err.message));
page.on('requestfailed', (req) =>
  console.log('[browser reqfail]', req.url(), req.failure()?.errorText),
);

// Stub __TAURI_INTERNALS__ the same way Tauri 2 does.
await page.evaluateOnNewDocument(() => {
  const stubInvoke = (cmd) => {
    console.log('[stub] invoke:', cmd);
    switch (cmd) {
      case 'get_lan_ip':
        return Promise.resolve('192.168.43.61');
      case 'get_port':
        return Promise.resolve(3131);
      case 'get_connected_devices':
        return Promise.resolve([]);
      case 'get_pending_device':
        return Promise.resolve(null);
      case 'get_app_language':
        return Promise.resolve('en');
      case 'get_is_first_time_start':
        return Promise.resolve(false);
      case 'get_current_version':
        return Promise.resolve('0.1.0');
      case 'check_screen_recording_permission':
        return Promise.resolve(true);
      default:
        return Promise.resolve(null);
    }
  };
  window.__TAURI_INTERNALS__ = {
    invoke: stubInvoke,
    transformCallback: () => 0,
    metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
  };
});

await page.goto('http://localhost:4173/', { waitUntil: 'networkidle0', timeout: 15000 });
await new Promise((r) => setTimeout(r, 2500));

const state = await page.evaluate(() => {
  const card = document.querySelector('.qr-card');
  const url = document.querySelector('.qr-url')?.textContent ?? null;
  const status = document.querySelector('.qr-status-text')?.textContent ?? null;
  const img = document.querySelector('.qr-img');
  return {
    cardDataState: card?.getAttribute('data-state') ?? null,
    url,
    status,
    qrImgPresent: !!img,
    qrImgSrcHead: img?.getAttribute('src')?.slice(0, 40) ?? null,
  };
});

console.log('QRCard state:', JSON.stringify(state, null, 2));
await browser.close();