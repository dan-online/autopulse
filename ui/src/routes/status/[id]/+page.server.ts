import { fail, redirect, type Actions } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = async (event) => {
    if (!event.locals.auth) {
        return redirect(302, "/login");
    }
    
    const { serverUrl, username, password } = event.locals.auth;
    
    const statsUrl = new URL(serverUrl);
    statsUrl.pathname = "/status/" + event.params.id

    const ev = await fetch(statsUrl, {
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

    if (!ev.ok) {
        return redirect(302, "/login");
    }

    return {ev: await ev.json()};
}

// export const actions: Actions = {
  
// }