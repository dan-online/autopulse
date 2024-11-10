import { env } from "$env/dynamic/private";
import { type Payload, verify } from "$lib/auth";
import { isForced } from "$lib/forced";
import type { Cookies, Handle } from "@sveltejs/kit";

const getAuthCookie = async (cookies: Cookies) => {
	if (isForced) {
		const username = env.FORCE_USERNAME || "";
		const password = env.FORCE_PASSWORD || "";
		const serverUrl = env.FORCE_SERVER_URL;

		if (!serverUrl) {
			throw new Error('FORCE_SERVER_URL is required when FORCE_AUTH is "true"');
		}

		return {
			serverUrl,
			username,
			password,
		} satisfies Payload;
	}

	const authCookie = cookies.get("auth");

	return authCookie ? await verify(authCookie).catch(() => null) : null;
};

export const handle: Handle = async ({ event, resolve }) => {
	event.locals.auth = await getAuthCookie(event.cookies);

	const start = performance.now();

	const result = await resolve(event);

	const end = performance.now();

	console.log(
		`${new Date().toISOString()} [${event.request.method}] - ${result.status} ${event.url.toString()} - ${(end - start).toFixed(3)}ms`,
	);

	return result;
};
