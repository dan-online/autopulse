import { redirect } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = async (event) => {
    if (!event.locals.auth) {
        return redirect(302, "/login");
    }
    
    const { serverUrl, username, password } = event.locals.auth;
    
    const statsUrl = new URL(serverUrl);
    statsUrl.pathname = "/stats"

    const stats = await fetch(statsUrl, {
        headers: {
            Authorization: 'Basic ' + btoa(username + ':' + password),
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
        return redirect(302, "/login");
    }

    const eventsUrl = new URL(serverUrl);
    eventsUrl.pathname = "/list"

    const events = await fetch(
        eventsUrl, {
        headers: {
            Authorization: 'Basic ' + btoa(username + ':' + password),
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
        return redirect(302, "/login");
    }

    return {stats: await stats.json(), events: await events.json()};
}
