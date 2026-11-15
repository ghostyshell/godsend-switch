// Render docs/og.png from the gold mark and the family palette.
import { webkit } from "@playwright/test";
import { readFileSync } from "node:fs";

const logo = readFileSync("src/assets/logo.png").toString("base64");
const html = `<!doctype html><html><body style="margin:0;width:1200px;height:630px;background:#04060c;color:#eef2fb;font-family:-apple-system,Segoe UI,sans-serif;display:grid;place-items:center">
<div style="text-align:center">
  <img src="data:image/png;base64,${logo}" style="width:150px;height:150px" />
  <div style="margin-top:18px;font:700 64px/1.1 -apple-system,Segoe UI,sans-serif">GODsend Switch</div>
  <div style="margin-top:14px;font:400 26px/1.4 -apple-system,Segoe UI,sans-serif;color:#a5b0c7">Library, installs, homebrew and a virtual pad over USB</div>
  <div style="margin-top:26px;font:500 18px/1 monospace;color:#56e8f5;letter-spacing:.18em">MTP · TINFOIL · LAN INSTALL</div>
</div></body></html>`;

const browser = await webkit.launch();
const page = await browser.newPage({ viewport: { width: 1200, height: 630 } });
await page.setContent(html, { waitUntil: "load" });
await page.screenshot({ path: "../docs/og.png" });
await browser.close();
console.log("rendered docs/og.png");
