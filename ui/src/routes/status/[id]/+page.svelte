<script lang="ts">
import { invalidateAll } from "$app/navigation";
import { page } from "$app/state";
import { onMount } from "svelte";

import { enhance } from "$app/forms";
import TimeAgo from "$lib/components/core/TimeAgo.svelte";

let ev = $derived(page.data.ev);

let updateTimeout: number;

async function autoReload() {
	await invalidateAll();
	updateTimeout = setTimeout(() => {
		autoReload();
	}, 500);
}

onMount(() => {
	autoReload();

	return () => {
		clearTimeout(updateTimeout);
	};
});
</script>

<div class="flex flex-col mt-6 gap-6">
    <div class="flex">
        <div class="mx-auto h-30 w-30 p-4 rounded-full bg-base-200">
            {#if !ev.file_path.endsWith("/")}
                 <i class="block i-ic-baseline-insert-drive-file w-full h-full text-primary"></i>
            {:else}
                 <i class="block i-ic-round-folder w-full h-full text-primary"></i>
            {/if}
        </div>
    </div>
    <ul class="steps">
        <li class="step step-primary">Added</li>
        <li
            class="step"
            class:step-primary={ev.found_status === "found" ||
                ev.process_status === "complete"}
            class:step-warning={ev.found_status === "hash_mismatch"}
        >
            Found
        </li>
        <li class="step" class:step-primary={ev.process_status === "complete"}>
            Processed
        </li>
    </ul>

    <div class="flex">
        <div class="card bg-base-200 shadow-xl w-full">
            <div class="card-body">
                <h2 class="card-title">Information</h2>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <div class="col-span-2">
                        <div class="text-primary">File Path</div>
                        <div class="break-all">{ev.file_path}</div>
                    </div>
                    {#if ev.file_hash}
                        <div class="col-span-2">
                            <div class="text-primary">File Hash</div>
                            <div class="break-all">{ev.file_hash}</div>
                        </div>
                    {/if}
                    <div>
                        <div class="text-primary">Timer</div>
                        <div>
                            <TimeAgo date={new Date(ev.can_process + "Z")} />
                        </div>
                    </div>
                    <div class:opacity-60={!ev.next_retry_at}>
                        <div class="text-primary">Next Retry</div>
                        <div>
                            {#if ev.next_retry_at}
                                <TimeAgo
                                    date={new Date(ev.next_retry_at + "Z")}
                                />
                            {:else}
                                N/A
                            {/if}
                        </div>
                    </div>
                    <div>
                        <div class="text-primary">Found Status</div>
                        <div>{ev.found_status}</div>
                    </div>
                    <div class:opacity-60={!ev.found_at}>
                        <div class="text-primary">Found Time</div>
                        <div>
                            {ev.found_at
                                ? new Date(ev.found_at + "Z").toLocaleString(
                                      "en-UK",
                                  )
                                : "N/A"}
                        </div>
                    </div>
                    <div>
                        <div class="text-primary">Process Status</div>
                        <div>{ev.process_status}</div>
                    </div>
                    <div class:opacity-60={!ev.processed_at}>
                        <div class="text-primary">Process Time</div>
                        <div>
                            {ev.processed_at
                                ? new Date(
                                      ev.processed_at + "Z",
                                  ).toLocaleString("en-UK")
                                : "N/A"}
                        </div>
                    </div>
                    <div>
                        <div class="text-primary">Created At</div>
                        <div>
                            {new Date(ev.created_at + "Z").toLocaleString(
                                "en-UK",
                            )}
                        </div>
                    </div>
                    <div>
                        <div class="text-primary">Updated At</div>
                        <div>
                            {new Date(ev.updated_at + "Z").toLocaleString(
                                "en-UK",
                            )}
                        </div>
                    </div>
                </div>
                <div class="card-actions justify-between mt-4">
                    <form action="/add?/add" method="post" use:enhance>
                        <input type="hidden" name="path" value={ev.file_path} />
                        <input type="hidden" name="hash" value={ev.file_hash} />
                        <input type="hidden" name="redirect" value="true" />
                        <button
                            type="submit"
                            class="btn btn-primary disabled:pointer-events-none"
                            disabled={ev.process_status !== "complete"}
                        >
                            Retry
                        </button>
                    </form>
                    <!-- <form method="post">
                        <button type="submit" class="btn btn-error">
                            Delete
                        </button>
                    </form> -->
                </div>
            </div>
        </div>
    </div>
</div>
