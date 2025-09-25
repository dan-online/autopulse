import { redirect } from "@sveltejs/kit";
import { resolve } from "$app/paths";
import { isForced } from "$lib/forced";
import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = async (event) => {
	const { serverUrl, username, password } = event.locals.auth!;

	const statsUrl = new URL(serverUrl);
	statsUrl.pathname = `/status/${event.params.id}`;

	const ev = await fetch(statsUrl, {
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

	if (!ev.ok) {
		if (ev.status === 401 && !isForced) {
			return redirect(302, resolve("/login"));
		}

		return {
			error: `${ev.statusText}: ${await ev.text()}`,
		};
	}

	return { ev: await ev.json() };
};

// export const actions: Actions = {

// }
