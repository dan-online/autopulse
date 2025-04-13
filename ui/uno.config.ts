import extractorSvelte from "@unocss/extractor-svelte";
import {
	defineConfig,
	presetIcons,
	presetTypography,
	presetWebFonts,
	presetWind3,
} from "unocss";
import { presetDaisy } from "unocss-preset-daisyui-next";

export default defineConfig({
	extractors: [extractorSvelte()],
	presets: [
		presetWind3(),
		presetIcons(),
		presetWebFonts({
			provider: "bunny",
			fonts: {
				base: "Inter:400,500,600,700,800,900",
				mono: "Roboto Mono:400,500,600,700,800,900",
			},
		}),
		presetDaisy({
			themes: ["night", "winter"],
		}),
		presetTypography(),
	],
});
