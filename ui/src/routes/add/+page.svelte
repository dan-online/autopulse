<script lang="ts">
import { enhance } from "$app/forms";
import type { ActionData } from "./$types";

export let form: ActionData;
let loading = false;

$: error = form?.error;
</script>

<svelte:head>
    <title>autopulse | add</title>
</svelte:head>

<div
    class="flex lg:w-2/3 mx-auto min-h-full flex-col justify-center px-6 py-12 lg:px-8"
>
    <div class="sm:mx-auto sm:w-full sm:max-w-sm">
        <h2 class="mt-10 text-center text-2xl font-bold">
            Manually add a new file/dir
        </h2>
    </div>

    {#if error}
        <div class="mt-4 bg-red-100 border-l-4 border-red-500 text-red-700 p-4">
            <p>{error.slice(0, 128)}</p>
        </div>
    {/if}

    {#if form?.success}
        <div
            class="mt-4 bg-green-100 border-l-4 border-green-500 text-green-700 p-4"
        >
            <p>
                Succesfully added <a
                    class="underline"
                    href={`/status/${form.event.id}`}
                >
                    {form.event.file_path}
                </a>
            </p>
        </div>
    {/if}

    <div class="mt-10 sm:mx-auto sm:w-full sm:max-w-sm">
        <form class="space-y-6" method="POST" action="?/add" use:enhance>
            <div>
                <label for="path"
                    >File/Dir Path<span class="text-red-500">*</span></label
                >
                <div class="mt-2">
                    <input
                        id="path"
                        name="path"
                        type="text"
                        required
                        class="input input-bordered w-full"
                    />
                </div>
            </div>

            <div>
                <label for="hash">Hash</label>
                <div class="mt-2">
                    <input
                        id="hash"
                        name="hash"
                        type="text"
                        class="input input-bordered w-full"
                    />
                </div>
            </div>

            <div>
                <button
                    type="submit"
                    disabled={loading}
                    class="btn btn-primary disabled:pointer-events-none disabled:grayscale"
                    >Submit</button
                >
            </div>
        </form>
    </div>
</div>
