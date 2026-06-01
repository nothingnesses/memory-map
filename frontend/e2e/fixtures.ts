import {
	expect,
	test as base,
} from "@playwright/test";

const transparentPng = Buffer.from(
	"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAFgwJ/lz7eSgAAAABJRU5ErkJggg==",
	"base64",
);

const test = base.extend({
	page: async ({ page }, use) => {
		const consoleErrors: string[] = [];
		const failedRequests: string[] = [];

		await page.route("https://tile.openstreetmap.org/**", async (route) => {
			await route.fulfill({
				status: 200,
				contentType: "image/png",
				body: transparentPng,
			});
		});

		page.on("console", (message) => {
			if (message.type() === "error") {
				consoleErrors.push(message.text());
			}
		});

		page.on("requestfailed", (request) => {
			const failure = request.failure();
			failedRequests.push(
				`${request.method()} ${request.url()} ${failure?.errorText ?? "failed"}`,
			);
		});

		await use(page);

		expect(consoleErrors).toEqual([]);
		expect(failedRequests).toEqual([]);
	},
});

export {
	expect,
	test,
};
