import cfAdapter from "@sveltejs/adapter-cloudflare";
import nodeAdapter from "@sveltejs/adapter-node";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),

	kit: {
		adapter: process.env.CF_PAGES ? cfAdapter() : nodeAdapter(),
	},
};

export default config;
