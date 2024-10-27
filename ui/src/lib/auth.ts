import { building } from "$app/environment";
import { env } from "$env/dynamic/private";
import { aesGcmDecrypt, aesGcmEncrypt } from "./encrypt";

const secret = env.SECRET;

if (!secret && !building) {
	throw new Error("SECRET must be defined");
}

export interface Payload {
	serverUrl: string;
	username: string;
	password: string;
}

export const sign = async (payload: Payload) => {
	const data = JSON.stringify(payload);
	const encrypted = aesGcmEncrypt(data, secret);

	return encrypted;
};

export const verify = async (jwt: string) => {
	const decrypted = await aesGcmDecrypt(jwt, secret);

	return JSON.parse(decrypted) as Payload;
};
