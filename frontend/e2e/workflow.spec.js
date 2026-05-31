const fs = require("node:fs");
const path = require("node:path");

const { expect, test } = require("./fixtures");

const fixtureBuffer = fs.readFileSync(
	path.join(__dirname, "fixtures", "memory-map-e2e.svg"),
);

const password = "memory-map-e2e-password-123";
const backendUrl = process.env.E2E_BACKEND_URL ?? "http://127.0.0.1:8000";

function runId() {
	return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

async function expectImageLoaded(locator) {
	await expect(locator).toBeVisible();
	await expect
		.poll(async () =>
			locator.evaluate(
				(image) => image.complete && image.naturalWidth > 0 && image.naturalHeight > 0,
			),
		)
		.toBe(true);
}

async function openMenu(page) {
	await page.getByRole("button", { name: "Open menu" }).click();
	await expect(page.getByRole("button", { name: "Close menu" })).toBeVisible();
}

async function openMarkerPopup(page, text) {
	const markers = page.locator(".leaflet-marker-icon");
	await expect(markers.first()).toBeVisible();

	const count = await markers.count();
	for (let index = 0; index < count; index += 1) {
		await markers.nth(index).click();
		const popupHeading = page.getByRole("heading", { name: text });
		if (await popupHeading.isVisible().catch(() => false)) {
			return popupHeading;
		}
	}

	throw new Error(`Could not find marker popup for ${text}`);
}

function waitForGraphqlOperation(page, operationName) {
	return page.waitForResponse((response) => {
		const request = response.request();
		return (
			request.method() === "POST" &&
			response.url().startsWith(backendUrl) &&
			request.postData()?.includes(operationName)
		);
	});
}

test("authenticated object workflow covers upload preview gallery delete and logout", async ({
	page,
}) => {
	const id = runId();
	const email = `memory-map-e2e-${id}@example.test`;
	const objectName = `memory-map-e2e-${id}.svg`;
	const latitude = "51.505";
	const longitude = "-0.09";
	const locationText = `${latitude}, ${longitude}`;

	await test.step("home page and anonymous menu load", async () => {
		await page.goto("/");
		await expect(page).toHaveTitle(/Memory Map/);
		await expect(page.locator(".leaflet-container")).toBeVisible();
		await openMenu(page);
		await expect(page.getByRole("link", { name: "Sign In" })).toBeVisible();
		await page.getByRole("button", { name: "Close menu" }).click();
	});

	await test.step("register and sign in", async () => {
		await page.goto("/register");
		await expect(page.getByRole("heading", { name: "Register" })).toBeVisible();
		await page.getByLabel("Email").fill(email);
		await page.getByLabel("Password", { exact: true }).fill(password);
		await page.getByLabel("Confirm Password").fill(password);
		await page.getByRole("button", { name: "Register" }).click();
		await expect(page).toHaveURL(/\/sign-in$/);

		await page.getByLabel("Email").fill(email);
		await page.getByLabel("Password", { exact: true }).fill(password);
		await page.getByRole("button", { name: "Sign In" }).click();
		await expect(page).toHaveURL(/\/$/);
	});

	await test.step("upload object and verify table preview", async () => {
		await page.goto("/objects");
		await expect(page.getByRole("heading", { name: "Objects" })).toBeVisible();
		await page.getByRole("button", { name: "Add Object" }).click();
		await expect(page.getByRole("heading", { name: "Add Object", exact: true })).toBeVisible();

		await page.getByLabel("Set latitude").fill(latitude);
		await page.getByLabel("Set longitude").fill(longitude);
		await page.getByLabel("Set date and time").fill("2026-05-29T12:34");
		await page.getByLabel("Select files to upload").setInputFiles({
			name: objectName,
			mimeType: "image/svg+xml",
			buffer: fixtureBuffer,
		});

		const uploadResponsePromise = page.waitForResponse(
			(response) =>
				response.request().method() === "POST" &&
				response.url().includes("/api/locations/"),
		);
		await page.getByRole("button", { name: "Submit" }).click();
		const uploadResponse = await uploadResponsePromise;
		expect(uploadResponse.ok()).toBe(true);
		expect(new URL(page.url()).search).toBe("");

		const row = page.getByRole("row").filter({ hasText: objectName });
		await expect(row).toBeVisible();
		await expect(row).toContainText(locationText);

		const thumbnail = row.getByRole("img", { name: objectName });
		await expectImageLoaded(thumbnail);

		await row.getByRole("button", { name: objectName }).click();
		const previewDialog = page
			.getByRole("dialog")
			.filter({ has: page.getByRole("img", { name: objectName }) })
			.last();
		await expect(previewDialog).toBeVisible();
		await expectImageLoaded(previewDialog.getByRole("img", { name: objectName }));
		await previewDialog.getByRole("button", { name: "Close" }).click();
		await expect(previewDialog).toBeHidden();
	});

	await test.step("verify map marker and gallery", async () => {
		await page.goto("/");
		await expect(page.locator(".leaflet-container")).toBeVisible();
		await openMarkerPopup(page, locationText);
		await page.getByRole("button", { name: "Open Gallery" }).click();

		const galleryDialog = page.getByRole("dialog").filter({ hasText: locationText });
		await expect(galleryDialog).toBeVisible();
		await expectImageLoaded(galleryDialog.getByRole("img", { name: objectName }).first());
		await galleryDialog.getByRole("button", { name: "Close" }).click();
		await expect(galleryDialog).toBeHidden();
	});

	await test.step("delete object and verify protected route logout behavior", async () => {
		await page.goto("/objects");
		const row = page.getByRole("row").filter({ hasText: objectName });
		await expect(row).toBeVisible();
		await row.getByRole("button", { name: "Delete" }).click();

		const confirmDialog = page.getByRole("dialog").filter({ hasText: objectName });
		await expect(confirmDialog).toBeVisible();
		await confirmDialog.getByRole("button", { name: "Yes" }).click();
		await expect(page.getByRole("row").filter({ hasText: objectName })).toHaveCount(0);

		await openMenu(page);
		const markersReloaded = waitForGraphqlOperation(page, "S3ObjectsQuery");
		await page.getByRole("button", { name: "Log Out" }).click();
		await expect(page).toHaveURL(/\/$/);
		expect((await markersReloaded).ok()).toBe(true);

		await page.goto("/objects");
		await expect(page).toHaveURL(/\/sign-in$/);
		await expect(page.getByRole("heading", { name: "Sign In" })).toBeVisible();
	});
});

test("mobile map and menu smoke", async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto("/");

	await expect(page.getByRole("heading", { name: "Map" })).toBeVisible();
	await expect(page.locator(".leaflet-container")).toBeVisible();
	await openMenu(page);
	await expect(page.getByRole("link", { name: "Sign In" })).toBeVisible();
	await page.getByRole("button", { name: "Close menu" }).click();
	await expect(page.getByRole("button", { name: "Open menu" })).toBeVisible();
});
