import {
	defineConfig,
	presetUno,
	presetWebFonts,
	presetTypography,
} from "unocss";
import extractorSvelte from "@unocss/extractor-svelte";
import { presetDaisy } from "@matthiesenxyz/unocss-preset-daisyui";

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
			themes: ["night", "winter"]
		}),
		presetTypography(),
	],
});
