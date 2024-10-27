import type { Payload } from "$lib/auth";
import "unplugin-icons/types/svelte";

declare global {
	namespace App {
		// interface Error {}
		interface Locals {
			auth: Payload | null;
		}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}
}
