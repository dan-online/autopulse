import { building } from "$app/environment";
import { env } from "$env/dynamic/private";
import { aesGcmDecrypt, aesGcmEncrypt } from "./encrypt";

let secret = env.SECRET;

if (!secret) {
    secret = crypto.randomUUID();
    
    console.log("Generated new secret:", secret);
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
