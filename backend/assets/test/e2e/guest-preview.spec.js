const { test, expect } = require("@playwright/test");

function createSilentWavBuffer({ durationSeconds, sampleRate = 44_100, channels = 1 }) {
  const frameCount = Math.max(1, Math.floor(durationSeconds * sampleRate));
  const bytesPerSample = 2;
  const blockAlign = channels * bytesPerSample;
  const byteRate = sampleRate * blockAlign;
  const dataSize = frameCount * blockAlign;
  const buffer = Buffer.alloc(44 + dataSize);

  buffer.write("RIFF", 0);
  buffer.writeUInt32LE(36 + dataSize, 4);
  buffer.write("WAVE", 8);
  buffer.write("fmt ", 12);
  buffer.writeUInt32LE(16, 16);
  buffer.writeUInt16LE(1, 20);
  buffer.writeUInt16LE(channels, 22);
  buffer.writeUInt32LE(sampleRate, 24);
  buffer.writeUInt32LE(byteRate, 28);
  buffer.writeUInt16LE(blockAlign, 32);
  buffer.writeUInt16LE(16, 34);
  buffer.write("data", 36);
  buffer.writeUInt32LE(dataSize, 40);

  return buffer;
}

test.beforeEach(async ({ page }) => {
  await page.route("**/api/jobs/current", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ job: null }),
    });
  });
});

test("guest preview happy path trims locally and shows completed preview", async ({ page }) => {
  await page.route("**/api/preview", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "audio/wav",
      body: createSilentWavBuffer({ durationSeconds: 3 }),
    });
  });

  await page.goto("/app");

  await page.locator("#audio-file-input").setInputFiles({
    name: "long-preview.wav",
    mimeType: "audio/wav",
    buffer: createSilentWavBuffer({ durationSeconds: 35 }),
  });

  await expect(page.getByText("Ready to munch!")).toBeVisible();
  await expect(page.getByText("Preview clipped from 0m 35s to 0m 30s")).toBeVisible();

  await page.getByRole("button", { name: "MUNCH IT!" }).click();

  await expect(page.getByText("Your audio is ready!")).toBeVisible();
  await expect(page.getByRole("button", { name: "Upload another file" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Download processed audio" })).toBeVisible();
});

test("guest busy preview shows friendly prompt instead of an error", async ({ page }) => {
  await page.route("**/api/preview", async (route) => {
    await route.fulfill({
      status: 503,
      contentType: "application/json",
      body: JSON.stringify({ error: "preview_busy" }),
    });
  });

  await page.goto("/app");

  await page.locator("#audio-file-input").setInputFiles({
    name: "short-preview.wav",
    mimeType: "audio/wav",
    buffer: createSilentWavBuffer({ durationSeconds: 5 }),
  });

  await expect(page.getByText("Preview clip ready: 0m 5s")).toBeVisible();

  await page.getByRole("button", { name: "MUNCH IT!" }).click();

  await expect(page.getByText("The demo is busy right now")).toBeVisible();
  await expect(page.getByText("Lots of people are trying the preview at the moment.")).toBeVisible();
  await expect(page.getByRole("link", { name: "Sign in" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Maybe later" })).toBeVisible();
});
