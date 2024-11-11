<script lang="ts">
import { applyAction, enhance } from "$app/forms";
import { goto } from "$app/navigation";
import { page } from "$app/stores";
import type { ActionData } from "./$types";

import SvgSpinners90RingWithBg from "~icons/svg-spinners/90-ring-with-bg";

export let form: ActionData;
let loading = false;

$: defaultURL = $page.data.defaultURL;
$: forceDefaultURL = $page.data.forceDefaultURL;
$: error = form?.error;
</script>

<div
    class="flex lg:w-2/3 mx-auto min-h-full flex-col justify-center px-6 py-12 lg:px-8"
>
    <div class="sm:mx-auto sm:w-full sm:max-w-sm">
        <h2 class="mt-10 text-center text-2xl font-bold">
            Sign in to autopulse
        </h2>
    </div>

    {#if error}
        <div class="mt-4 bg-red-100 border-l-4 border-red-500 text-red-700 p-4">
            <p>{error.slice(0, 128)}</p>
        </div>
    {/if}

    <div class="mt-10 sm:mx-auto sm:w-full sm:max-w-sm">
        <form
            class="space-y-6"
            method="POST"
            use:enhance={() => {
                loading = true;

                return async ({ result }) => {
                    if (result.type === "redirect") {
                        await goto(result.location);
                    } else {
                        await applyAction(result);
                    }

                    loading = false;
                };
            }}
        >
            <div>
                <label for="server-url">Server URL</label>
                <div class="mt-2">
                    <input
                        id="server-url"
                        name="server-url"
                        type="url"
                        value={defaultURL}
                        readonly={forceDefaultURL}
                        required
                        class="input input-bordered w-full read-only:opacity-50 read-only:pointer-events-none"
                    />
                </div>
            </div>

            <div>
                <label for="username">Username</label>
                <div class="mt-2">
                    <input
                        id="username"
                        name="username"
                        type="username"
                        required
                        class="input input-bordered w-full"
                    />
                </div>
            </div>

            <div>
                <div class="flex items-center justify-between">
                    <label for="password">Password</label>
                </div>
                <div class="mt-2">
                    <input
                        id="password"
                        name="password"
                        type="password"
                        required
                        class="input input-bordered w-full"
                    />
                </div>
            </div>

            <div>
                <button
                    type="submit"
                    disabled={loading}
                    class="btn w-32 btn-primary disabled:pointer-events-none disabled:grayscale"
                >
                    {#if !loading}
                        <span>Sign in</span>
                    {:else}
                        <SvgSpinners90RingWithBg class="w-6 h-6" />
                    {/if}
                </button>
            </div>
        </form>
    </div>
</div>
