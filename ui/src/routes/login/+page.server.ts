import { type Actions, fail, redirect } from "@sveltejs/kit";
import { env } from "$env/dynamic/private";
import { sign } from "$lib/auth";
import { isForced } from "$lib/forced";
import type { PageServerLoad } from "./$types";

const getURLOptions = (url: URL) => {
	const currentDefaultURL = new URL(url);

	currentDefaultURL.pathname = "";
	currentDefaultURL.search = "";

	const defaultURL = env.DEFAULT_SERVER_URL
		? new URL(env.DEFAULT_SERVER_URL)
		: currentDefaultURL;
	const forceDefaultURL = env.FORCE_DEFAULT_SERVER_URL === "true";

	return [defaultURL, forceDefaultURL] as const;
};

export const load: PageServerLoad = async ({ url, cookies }) => {
	if (isForced) {
		return redirect(302, "/");
	}

	const [defaultURL, forceDefaultURL] = getURLOptions(url);

	cookies.delete("auth", {
		path: "/",
	});

	return {
		defaultURL: defaultURL.href,
		forceDefaultURL: forceDefaultURL,
	};
};

export const actions: Actions = {
	default: async ({ request, cookies, url }) => {
		const formData = await request.formData();

		const [defaultURL, forceDefaultURL] = getURLOptions(url);

		// Force default URL if the environment variable is set
		const serverUrl = forceDefaultURL
			? defaultURL
			: (formData.get("server-url") as string);
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
					serverUrl: serverUrl.toString(),
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
