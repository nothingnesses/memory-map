import {
	expect,
	test,
} from "./fixtures";

test("home page loads the map shell", async ({ page }) => {
	await page.goto("/");

	await expect(page).toHaveTitle(/Memory Map/);
	await expect(page.locator(".leaflet-container")).toBeVisible();
	await expect(page.getByRole("button", { name: "Open menu" })).toBeVisible();
});
