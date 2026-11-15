// Regenerate the site screenshots from the mock bridge (WebKit, like Tauri).
// Usage: npm run dev (separately) then: node scripts/capture-screens.mjs
import { webkit } from "@playwright/test";

const BASE = process.env.BASE_URL ?? "http://localhost:1420";
const browser = await webkit.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
await page.goto(`${BASE}/?mock=1`, { waitUntil: "load", timeout: 30000 });
await page.waitForTimeout(1800);
const shots = [
  ["Console", "site-console.png"],
  ["Devices", "site-devices.png"],
  ["Library", "site-library.png"],
  ["Controller", "site-controller.png"],
  ["Game Index", "site-store.png"],
  ["Downloads", "site-downloads.png"],
  ["Files", "site-files.png"],
  ["Convert", "site-convert.png"],
  ["Homebrew", "site-homebrew.png"],
  ["Settings", "site-settings.png"],
];
for (const [nav, file] of shots) {
  if (nav !== "Console") {
    await page.getByRole("button", { name: nav, exact: true }).first().click();
    await page.waitForTimeout(1200);
  }
  await page.screenshot({ path: `../docs/${file}` });
  console.log("captured", file);
}
await browser.close();
