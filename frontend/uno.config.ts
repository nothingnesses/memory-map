import presetWind4 from "@unocss/preset-wind4";
import { defineConfig } from "unocss";

export default defineConfig({
	cli: {
		entry: [
			{
				patterns: ["index.html", "src/**/*.rs"],
				outFile: "style/uno.css",
			},
		],
	},
	presets: [presetWind4()],
});
