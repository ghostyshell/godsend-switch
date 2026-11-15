import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/?mock=1", { waitUntil: "load" });
  await page.waitForTimeout(1500);
});

async function nav(page: import("@playwright/test").Page, name: string) {
  await page.getByRole("button", { name, exact: true }).click();
}

test("shell landmarks and nav", async ({ page }) => {
  await expect(page.locator("header.topbar")).toBeVisible();
  await expect(page.locator("nav.rail")).toBeVisible();
  await expect(page.locator("footer.statusbar")).toBeVisible();
  await expect(page.getByRole("link", { name: "Skip to content" })).toHaveCount(1);
  await expect(page.locator('.rail-item[aria-current="page"]')).toHaveText(/Console/);
  await expect(page.getByRole("heading", { name: "Nintendo Switch" })).toBeVisible();
});

test("library lists the installed fixtures", async ({ page }) => {
  await nav(page, "Library");
  await expect(page.locator(".tile")).toHaveCount(4);
  await expect(page.locator(".tile .t").first()).toContainText(/Mario Kart 8 Deluxe/i);
});

test("library switches to local packages", async ({ page }) => {
  await nav(page, "Library");
  await page.getByRole("tab", { name: "Local" }).click();
  await expect(page.locator(".tile")).toHaveCount(2);
  await expect(page.locator(".tile .t").first()).toContainText(/Metroid Dread/i);
});

test("devices shows USB and the FTP host", async ({ page }) => {
  await nav(page, "Devices");
  await expect(page.getByText("MTP attached")).toBeVisible();
  await expect(page.getByText("Living room Switch").first()).toBeVisible();
  await expect(page.getByRole("button", { name: "Scan subnet" })).toBeVisible();
});

test("controller exposes the pad", async ({ page }) => {
  await nav(page, "Controller");
  await expect(page.getByRole("heading", { name: "Virtual controller" })).toBeVisible();
  await expect(page.getByRole("button", { name: "A", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Detach pad" })).toBeVisible();
});

test("convert explains the actions", async ({ page }) => {
  await nav(page, "Convert");
  await expect(page.locator(".notes").getByText("Split for FAT32")).toBeVisible();
  await expect(page.locator(".notes").getByText("Merge parts")).toBeVisible();
  await expect(page.getByText("Not supported")).toBeVisible();
});

test("homebrew catalog renders install buttons", async ({ page }) => {
  await nav(page, "Homebrew");
  await expect(page.locator(".store-card")).toHaveCount(6);
  await expect(page.locator(".store-card .name").filter({ hasText: "sys-con" })).toBeVisible();
});

test("downloads lists local packages", async ({ page }) => {
  await nav(page, "Downloads");
  await expect(page.locator(".file-row .name").filter({ hasText: "Metroid Dread" }).first()).toBeVisible();
});

test("game index search wires the source", async ({ page }) => {
  await nav(page, "Game Index");
  await page.getByPlaceholder("Search Switch games").fill("mario");
  await page.getByRole("button", { name: "Search" }).click();
  await expect(page.getByText("Mario Kart 8 Deluxe").first()).toBeVisible();
});

test("settings exposes transport and MCP controls", async ({ page }) => {
  await nav(page, "Settings");
  await expect(page.getByText("Install target")).toBeVisible();
  await expect(page.getByRole("heading", { name: "MCP server" })).toBeVisible();
  await expect(page.getByText("Start the MCP server with the app")).toBeVisible();
});

test("inspector queue shows the seeded jobs", async ({ page }) => {
  await expect(page.locator(".queue-card .job-row")).toHaveCount(2);
});
