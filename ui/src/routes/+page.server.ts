import { resolve } from "$app/paths";
import { isForced } from "$lib/forced";
import { redirect } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = async (event) => {
	const { serverUrl, username, password } = event.locals.auth!;

	const statsUrl = new URL(serverUrl);
	statsUrl.pathname = "/stats";

	const stats = await fetch(statsUrl, {
		headers: {
			Authorization: `Basic ${btoa(`${username}:${password}`)}`,
		},
	}).catch((err) => {
		return {
			ok: false as false,
			statusText: err.message,
			status: 500,
			text: async () => "Unknown error",
		};
	});

	if (!stats.ok) {
		if (stats.status === 401 && !isForced) {
			return redirect(302, resolve("/login"));
		}

		return {
			error: `${stats.statusText}: ${await stats.text()}`,
		};
	}

	const eventsUrl = new URL(serverUrl);
	eventsUrl.pathname = "/list";

	if (event.url.searchParams.has("sort")) {
		eventsUrl.searchParams.set("sort", event.url.searchParams.get("sort")!);
	}

	if (event.url.searchParams.has("search")) {
		eventsUrl.searchParams.set("search", event.url.searchParams.get("search")!);
	}

	if (event.url.searchParams.has("limit")) {
		eventsUrl.searchParams.set("limit", event.url.searchParams.get("limit")!);
	}

	if (event.url.searchParams.has("page")) {
		eventsUrl.searchParams.set("page", event.url.searchParams.get("page")!);
	}

	if (event.url.searchParams.has("status")) {
		eventsUrl.searchParams.set("status", event.url.searchParams.get("status")!);
	}

	const events = await fetch(eventsUrl, {
		headers: {
			Authorization: `Basic ${btoa(`${username}:${password}`)}`,
		},
	}).catch((err) => {
		return {
			ok: false as false,
			statusText: err.message,
			status: 500,
			text: async () => "Unknown error",
		};
	});

	if (!events.ok) {
		if (events.status === 401 && !isForced) {
			return redirect(302, resolve("/login"));
		}

		return {
			error: `${events.statusText} - ${await events.text()}`,
		};
	}

	const statsJson = await stats.json();
	const eventsJson = await events.json();

	return { stats: statsJson, events: eventsJson };
};
