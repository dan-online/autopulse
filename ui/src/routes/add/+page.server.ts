import { error, fail, redirect, type Actions } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = async ({ locals }) => {
    if (!locals.auth) {
        return redirect(302, "/login");
    }

    return {
    };
};

export const actions: Actions = {
    add: async ({ request, cookies, url, locals }) => {
        if (!locals.auth) {
            return redirect(302, "/login");
        }

        const { serverUrl, username, password } = locals.auth;

        const formData = await request.formData();
        
        const path = formData.get('path') as string;
        const hash = formData.get('hash') as string | null;

        const postUrl = new URL(serverUrl);

        postUrl.pathname = "/triggers/manual";
        postUrl.searchParams.set('path', path);

        if (hash) {
            postUrl.searchParams.set('hash', hash);
        }

        const response = await fetch(postUrl.href, {
            method: 'GET',
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

        if (response.ok) {
            return {
                success: true,
                event: await response.json()
            }
        } else {
            return fail(response.status, {
                error: response.statusText + ": " + await response.text()
            });
        }
    },
};