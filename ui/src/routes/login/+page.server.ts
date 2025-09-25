import { type Actions, redirect } from "@sveltejs/kit";
import { resolve } from "$app/paths";
import { env } from "$env/dynamic/private";
import { sign } from "$lib/auth";
import { isForced } from "$lib/forced";
import type { PageServerLoad } from "./$types";

const getURLOptions = () => {
	const defaultURL = env.DEFAULT_SERVER_URL
		? new URL(env.DEFAULT_SERVER_URL)
		: null;
	const forceDefaultURL = env.FORCE_DEFAULT_SERVER_URL === "true";

	return [defaultURL, forceDefaultURL] as const;
};

export const load: PageServerLoad = async ({ cookies }) => {
	if (isForced) {
		return redirect(302, resolve("/"));
	}

	const [defaultURL, forceDefaultURL] = getURLOptions();

	cookies.delete("auth", {
		path: "/",
	});

	return {
		defaultURL: defaultURL?.href,
		forceDefaultURL: forceDefaultURL,
	};
};

export const actions: Actions = {
	default: async ({ request, cookies, url }) => {
		const formData = await request.formData();

		const [defaultURL, forceDefaultURL] = getURLOptions();

		// Force default URL if the environment variable is set
		const serverUrl =
			forceDefaultURL && defaultURL
				? defaultURL
				: new URL(formData.get("server-url") as string);

		const username = formData.get("username") as string;
		const password = formData.get("password") as string;

		const postUrl = serverUrl;

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

			return redirect(302, resolve("/"));
		}

		return {
			error: `${response.statusText}: ${await response.text()}`,
		};
	},
};
