// Simulate the Tauri WebView's environment: load the vite dev URL with a
// stubbed @tauri-apps/api/core that returns real host-info (proxied from the
// running axum server). This is the closest headless approximation of what
// the real Tauri WebView does.
import puppeteer from 'puppeteer-core';
import http from 'node:http';

const VITE_URL = 'http://localhost:5173/';

// Pull real host-info from the running axum server.
const realHostInfo = await new Promise((resolve) => {
  http
    .get('http://127.0.0.1:3131/api/host-info', (r) => {
      let b = '';
      r.on('data', (c) => (b += c));
      r.on('end', () => resolve(JSON.parse(b)));
    })
    .on('error', () => resolve(null));
});
console.log('real host-info from axum:', realHostInfo);

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

// Inject IPC stub before any script runs.
await p.evaluateOnNewDocument((hostInfo) => {
  // Pretend to be Tauri IPC.
  const handlers = {
    get_lan_ip: () => hostInfo?.lan_ip ?? null,
    get_port: () => hostInfo?.port ?? 3131,
    get_connected_devices: () => [],
    get_pending_device: () => null,
    check_wifi_connection: () => true,
    check_screen_recording_permission: () => true,
    get_app_language: () => 'en',
    get_is_first_time_start: () => true,
    get_current_version: () => '0.1.0-test',
    enumerate_capture_sources: () => [],
    create_waiting_session: () => 'e2e',
    reset_waiting_session: () => undefined,
    set_capture_target: () => undefined,
    set_app_language: () => undefined,
    set_app_started_once: () => undefined,
    start_sharing: () => undefined,
    disconnect_all_devices: () => undefined,
    disconnect_device: () => true,
    is_viewer_slot_available: () => true,
    get_waiting_source_id: () => null,
    set_desktop_capturer_source_id: () => undefined,
    set_device_connected_status: () => undefined,
    relaunch_app: () => undefined,
    write_text_to_clipboard: () => undefined,
    open_external_link: () => undefined,
  };
  // Monkey-patch the global so @tauri-apps/api/core invoke routes here.
  // The bundled code does: const { invoke } = await import('@tauri-apps/api/core')
  // So we can't easily intercept that without the real module. Instead, expose
  // __TAURI_INTERNALS__ which is the runtime injection point in Tauri 2.
  (window).__TAURI_INTERNALS__ = {
    invoke: (cmd, _args) => {
      if (handlers[cmd]) return Promise.resolve(handlers[cmd]());
      console.warn('[stub] no handler for', cmd);
      return Promise.resolve(undefined);
    },
  };
}, realHostInfo);

await p.goto(VITE_URL, { waitUntil: 'networkidle2', timeout: 20000 });
await new Promise((r) => setTimeout(r, 3500));
const probe = await p.evaluate(() => {
  const url = document.querySelector('.qr-url')?.textContent?.trim();
  const empty = !!document.querySelector('.qr-empty');
  const img = document.querySelector('img.qr-img');
  const status = document.querySelector('.qr-status-text')?.textContent?.trim();
  return { url, empty, hasImg: !!img, status };
});
console.log('PROBE:', JSON.stringify(probe, null, 2));
await p.screenshot({
  path: '/Users/blxie/workspace/every-screen/screenmirror/tools/output/host-tauri-sim.png',
});
await b.close();
console.log('done');