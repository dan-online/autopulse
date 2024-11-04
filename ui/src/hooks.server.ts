import { verify } from "$lib/auth";
import type { Handle } from "@sveltejs/kit";

export const handle: Handle = async ({ event, resolve }) => {
	const authCookie = event.cookies.get("auth");

	event.locals.auth = authCookie
		? await verify(authCookie).catch(() => null)
		: null;

	const start = performance.now();
	
	const result = await resolve(event);

	const end = performance.now();

	console.log(`${new Date().toISOString()} [${event.request.method}] - ${result.status} ${event.url.toString()} - ${(end - start).toFixed(3)}ms`);

	return result;
};
