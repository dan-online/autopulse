import type { Handle } from "@sveltejs/kit";

export const handle: Handle = async ({ event, resolve }) => {
    event.locals.auth = event.cookies.get('auth') ? JSON.parse(event.cookies.get('auth')!) : null;

    return resolve(event);
}