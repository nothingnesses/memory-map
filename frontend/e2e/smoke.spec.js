const { expect, test } = require("@playwright/test");

test("home page loads the map shell", async ({ page }) => {
	const consoleErrors = [];
	const failedRequests = [];

	page.on("console", (message) => {
		if (message.type() === "error") {
			consoleErrors.push(message.text());
		}
	});

	page.on("requestfailed", (request) => {
		const failure = request.failure();
		failedRequests.push(`${request.method()} ${request.url()} ${failure?.errorText ?? "failed"}`);
	});

	await page.goto("/");

	await expect(page).toHaveTitle(/Memory Map/);
	await expect(page.locator(".leaflet-container")).toBeVisible();
	await expect(page.getByRole("button", { name: "Open menu" })).toBeVisible();

	expect(consoleErrors).toEqual([]);
	expect(failedRequests).toEqual([]);
});
