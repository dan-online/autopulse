import cfAdapter from "@sveltejs/adapter-cloudflare";
import nodeAdapter from "@sveltejs/adapter-node";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

// Base path must start with a slash and must not end with a slash
function fixBasePath(path) {
	let newPath = path?.trim();

	if (!newPath || newPath.length === 0) return newPath;
	if (!newPath.startsWith("/")) newPath = `/${newPath}`;
	if (newPath.endsWith("/")) newPath = newPath.slice(0, -1);

	return newPath;
}

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),

	kit: {
		adapter: process.env.CF_PAGES ? cfAdapter() : nodeAdapter(),
		paths: {
			base: fixBasePath(process.env.BASE_PATH) || "",
		},
	},
};

export default config;
