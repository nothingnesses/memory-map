import {
	expect,
	test as base,
} from "@playwright/test";

// Strict by default. Negative tests can opt out only for expected messages:
// test.use({ allowFailedRequests: [/PUT .*storage/] });
type StrictPageOptions = {
	allowConsoleErrors: RegExp[];
	allowFailedRequests: RegExp[];
};

const transparentPng = Buffer.from(
	"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAFgwJ/lz7eSgAAAABJRU5ErkJggg==",
	"base64",
);
const storageUrl = process.env.E2E_STORAGE_URL ?? "http://127.0.0.1:9000";

function allowedBy(
	value: string,
	allowed: RegExp[],
): boolean {
	return allowed.some((pattern) => pattern.test(value));
}

const test = base.extend<StrictPageOptions>({
	allowConsoleErrors: [[], { option: true }],
	allowFailedRequests: [[], { option: true }],
	page: async (
		{
			allowConsoleErrors,
			allowFailedRequests,
			page,
		},
		use,
	) => {
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
			if (
				request.method() === "PUT" &&
				request.url().startsWith(storageUrl) &&
				failure?.errorText === "net::ERR_ABORTED"
			) {
				return;
			}
			failedRequests.push(
				`${request.method()} ${request.url()} ${failure?.errorText ?? "failed"}`,
			);
		});

		await use(page);

		expect(consoleErrors.filter((message) => !allowedBy(message, allowConsoleErrors)))
			.toEqual([]);
		expect(failedRequests.filter((request) => !allowedBy(request, allowFailedRequests)))
			.toEqual([]);
	},
});

export {
	expect,
	test,
};
