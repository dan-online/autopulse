import { env } from "$env/dynamic/private";

export const isForced = env.FORCE_AUTH === "true";
