import cfAdapter from "@sveltejs/adapter-cloudflare";
import nodeAdapter from "@sveltejs/adapter-node";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

// Base path must start with a slash and must not end with a slash
function fixBasePath(path) {
	let newPath = path?.trim();

	if (!newPath) return newPath;
	if (newPath.length === 0) return path;
	if (!newPath.startsWith("/")) newPath = `/${path}`;
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
