import { type Actions, error, fail, redirect } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";
import { sign } from "$lib/auth";

export const load: PageServerLoad = async ({ url, cookies }) => {
	const defaultURL = new URL(url);

	defaultURL.pathname = "";
	defaultURL.search = "";

	cookies.delete("auth", {
		path: "/",
	});

	return {
		defaultURL: defaultURL.href,
	};
};

export const actions: Actions = {
	check: async ({ request, cookies, url }) => {
		const formData = await request.formData();

		const serverUrl = formData.get("server-url") as string;
		const username = formData.get("username") as string;
		const password = formData.get("password") as string;

		const postUrl = new URL(serverUrl);
		postUrl.pathname = "/login";

		const response = await fetch(postUrl.href, {
			method: "POST",
			headers: {
				Authorization: `Basic ${btoa(`${username}:${password}`)}`,
			},
		}).catch((err) => {
			return {
				ok: false,
				statusText: err.message,
				status: 500,
				text: async () => "Unknown error",
			};
		});

		if (response.ok) {
			cookies.set(
				"auth",
				await sign({
					serverUrl,
					username,
					password,
				}),
				{
					maxAge: 7 * 24 * 60 * 60,
					path: "/",
					secure: url.protocol === "https:",
				},
			);

			return redirect(302, "/");
		}

		return fail(response.status, {
			error: `${response.statusText}: ${await response.text()}`,
		});
	},
};
