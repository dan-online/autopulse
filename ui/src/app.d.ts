import 'unplugin-icons/types/svelte'

declare global {
	namespace App {
		// interface Error {}
		interface Locals {
			auth: {
				serverUrl: string;
				username: string;
				password: string;
			}
		}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}
}

export {};
