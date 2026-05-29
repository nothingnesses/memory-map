const { defineConfig, devices } = require("@playwright/test");

const frontendUrl = process.env.E2E_FRONTEND_URL ?? "http://127.0.0.1:3000";

module.exports = defineConfig({
	testDir: "./e2e",
	timeout: 60_000,
	expect: {
		timeout: 10_000,
	},
	fullyParallel: false,
	workers: 1,
	reporter: [
		["list"],
		["html", { open: "never" }],
	],
	use: {
		baseURL: frontendUrl,
		trace: "retain-on-failure",
		screenshot: "only-on-failure",
		video: "retain-on-failure",
		actionTimeout: 10_000,
		navigationTimeout: 30_000,
	},
	projects: [
		{
			name: "chromium",
			use: { ...devices["Desktop Chrome"] },
		},
	],
});
