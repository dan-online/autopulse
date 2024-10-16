<script setup lang="ts">
    import { page } from "$app/stores";
    import MaterialSymbolsFileCopyOutlineRounded from "~icons/material-symbols/file-copy-outline-rounded";
    import CiSearchMagnifyingGlass from "~icons/ci/search-magnifying-glass";
    import HugeiconsPackageDelivered from "~icons/hugeicons/package-delivered";
    import PajamasRetry from "~icons/pajamas/retry";
    import MaterialSymbolsError from "~icons/material-symbols/error";
    import { onMount, type ComponentType } from "svelte";
    import {
        goto,
        invalidateAll,
        preloadCode,
        preloadData,
        pushState,
    } from "$app/navigation";

    const iconMap: Record<string, ComponentType> = {
        total: MaterialSymbolsFileCopyOutlineRounded,
        found: CiSearchMagnifyingGlass,
        processed: HugeiconsPackageDelivered,
        retrying: PajamasRetry,
        failed: MaterialSymbolsError,
    };
    const descMap: Record<string, string> = {
        total: "Total scan events",
        found: "Found + Matched Hash",
        processed: "Sent to processors",
        retrying: "Failed 1/+ processors",
        failed: "Failed to process",
    };

    $: stats = $page.data.stats;
    $: events = $page.data.events;

    onMount(() => {
        const interval = setInterval(() => {
            invalidateAll();
        }, 5000);

        return () => clearInterval(interval);
    });
</script>

<div class="flex flex-col md:flex-row mt-4">
    {#each Object.entries(stats.stats) as [key, val], idx}
        <div class="stat" class:md:border-l={idx !== 0}>
            <div class="stat-figure text-primary">
                <svelte:component
                    this={iconMap[key]}
                    class="mt-4 lg:mt-0 inline-block h-8 w-8"
                />
            </div>
            <div class="stat-title">{key[0].toUpperCase() + key.slice(1)}</div>
            <div class="stat-value text-primary">{val}</div>
            <div class="hidden lg:block stat-desc">
                {descMap[key] || ""}
            </div>
        </div>
    {/each}
</div>

<div class="flex flex-col md:flex-row mt-4">
    <div class="card bg-base-200 shadow-xl">
        <div class="card-body">
            <h2 class="card-title">Events</h2>
            <div class="overflow-x-auto">
                <table class="table text-left">
                    <thead>
                        <tr>
                            <th></th>
                            <th>Path</th>
                            <th>Status</th>
                            <th>Trigger</th>
                            <th>Added At</th>
                            <th>Updated At</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each events as event}
                            <tr
                                class="cursor-pointer hover:bg-base-300 rounded-md"
                                on:click={() => goto(`/status/${event.id}`)}
                            >
                                <th>{event.id.split("-")[0]}</th>
                                <td>{event.file_path}</td>
                                <td>{event.process_status}</td>

                                <td>{event.event_source}</td>
                                <td
                                    >{new Date(
                                        event.created_at + "Z",
                                    ).toLocaleString("en-UK")}</td
                                >
                                <td
                                    >{new Date(
                                        event.updated_at + "Z",
                                    ).toLocaleString("en-UK")}</td
                                >
                                <td>
                                    <a
                                        href={`/status/${event.id}`}
                                        class="btn btn-sm btn-primary"
                                    >
                                        View
                                    </a>
                                </td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
        </div>
    </div>
</div>

<style>
    th,
    td {
        padding: 0.5rem;
    }
</style>
