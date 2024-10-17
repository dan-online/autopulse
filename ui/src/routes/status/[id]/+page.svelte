<script lang="ts">
    import { page } from "$app/stores";
    import TimeAgo from "$lib/components/core/TimeAgo.svelte";

    import IcBaselineInsertDriveFile from "~icons/ic/baseline-insert-drive-file";
    import IcRoundFolder from "~icons/ic/round-folder";

    $: ev = $page.data.ev;
</script>

<div class="flex flex-col mt-6 gap-6">
    <div class="flex">
        <div class="mx-auto h-30 w-30 p-4 rounded-full bg-base-200">
            {#if !ev.file_path.endsWith("/")}
                <IcBaselineInsertDriveFile class="w-full h-full text-primary" />
            {:else}
                <IcRoundFolder class="w-full h-full text-primary" />
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
                        <div>{ev.file_path}</div>
                    </div>
                    <div>
                        <div class="text-primary">Timer</div>
                        <div>
                            <TimeAgo date={new Date(ev.can_process + "Z")} />
                        </div>
                    </div>
                    <div>
                        <div class="text-primary">Found Status</div>
                        <div>{ev.found_status}</div>
                    </div>
                    <div>
                        <div class="text-primary">Process Status</div>
                        <div>{ev.process_status}</div>
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
            </div>
        </div>
    </div>
</div>
