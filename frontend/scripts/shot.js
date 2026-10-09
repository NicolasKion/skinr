// Opens a page of the running app in Chromium, prints console errors and saves a
// screenshot. Headless Chromium has no WebGPU adapter on most Linux setups, so
// this opens a window. Usage: node scripts/shot.js <path> <out.png> [waitForViewport]
import { chromium } from 'playwright';

const [path = '/', out = 'shot.png', waitForViewport] = process.argv.slice(2);
const base = process.env.BASE_URL ?? 'http://localhost:5200';

const browser = await chromium.launch({
  executablePath: process.env.CHROMIUM ?? '/usr/bin/chromium',
  headless: false,
  args: [
    '--enable-unsafe-webgpu',
    '--enable-features=Vulkan',
    '--use-angle=vulkan',
    '--ignore-gpu-blocklist',
  ],
});
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
page.on('console', (message) => {
  if (message.type() === 'error' || message.type() === 'warning')
    console.log(`${message.type()}: ${message.text().slice(0, 300)}`);
});
page.on('pageerror', (error) => console.log('pageerror:', error.message));
await page.goto(base + path);
if (waitForViewport) {
  // The viewport's status line disappears once the ship is built.
  await page.waitForSelector('.viewport canvas', { timeout: 90000 });
  await page.waitForFunction(() => !document.querySelector('.hud-frame .animate-spin'), null, {
    timeout: 90000,
  });
  await page.waitForTimeout(4000);
}
await page.screenshot({ path: out });
await browser.close();
