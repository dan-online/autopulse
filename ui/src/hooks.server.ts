import { verify } from "$lib/auth";
import type { Handle } from "@sveltejs/kit";

export const handle: Handle = async ({ event, resolve }) => {
	const authCookie = event.cookies.get("auth");

	event.locals.auth = authCookie
		? await verify(authCookie).catch(() => null)
		: null;

	return resolve(event);
};
