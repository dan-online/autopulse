import { sveltekit } from "@sveltejs/kit/vite";
import UnoCSS from "unocss/vite";
import Icons from "unplugin-icons/vite";
import { defineConfig } from "vite";

export default defineConfig({
	plugins: [
		UnoCSS(),
		sveltekit(),
		Icons({
			compiler: "svelte",
		}),
	],
});
