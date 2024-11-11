import { isForced } from "$lib/forced";
import type { LayoutServerLoad } from "./$types";

export const load: LayoutServerLoad = async (event) => {
	const setColorMode = event.url.searchParams.get("colorMode");

	if (setColorMode && ["dark", "light"].includes(setColorMode)) {
		event.cookies.set("colorMode", setColorMode, {
			path: "/",
			secure: event.url.protocol === "https:",
		});
	}

	return {
		forceAuth: isForced,
		colorMode: event.cookies.get("colorMode") || "dark",
	};
};
