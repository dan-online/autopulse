import { presetDaisy } from "@matthiesenxyz/unocss-preset-daisyui";
import extractorSvelte from "@unocss/extractor-svelte";
import {
	defineConfig,
	presetTypography,
	presetUno,
	presetWebFonts,
} from "unocss";

export default defineConfig({
	extractors: [extractorSvelte()],
	presets: [
		presetUno(),
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
