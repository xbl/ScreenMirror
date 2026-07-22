// Diagnostic: simulate a real Tauri WebView where IPC is NOT stubbed
// (i.e. what happens when the user just runs the binary without any
// instrumentation). We need to confirm whether the build really works
// with native Tauri IPC, or whether something about the build process
// or the @tauri-apps/api/core module is broken.
import puppeteer from 'puppeteer-core';

const browser = await puppeteer.launch({
  executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-setuid-sandbox'],
});
const page = await browser.newPage();

page.on('console', (msg) => console.log(`[browser ${msg.type()}]`, msg.text()));
page.on('pageerror', (err) => console.log('[browser pageerror]', err.message));

// Inspect what __TAURI_INTERNALS__ actually looks like in dist's runtime
await page.evaluateOnNewDocument(() => {
  // No stub — let dist's own logic run.
});

// Open the built bundle
await page.goto('http://localhost:4173/', { waitUntil: 'networkidle0', timeout: 15000 });
await new Promise((r) => setTimeout(r, 3000));

const state = await page.evaluate(() => {
  return {
    hasTauriInternals: typeof window.__TAURI_INTERNALS__,
    hasTauri: typeof window.__TAURI__,
    hasInvoke: typeof window.__TAURI__?.invoke ?? typeof window.__TAURI_INTERNALS__?.invoke,
    cardDataState: document.querySelector('.qr-card')?.getAttribute('data-state') ?? null,
    url: document.querySelector('.qr-url')?.textContent ?? null,
    status: document.querySelector('.qr-status-text')?.textContent ?? null,
  };
});

console.log('Without stub — state:', JSON.stringify(state, null, 2));
await browser.close();