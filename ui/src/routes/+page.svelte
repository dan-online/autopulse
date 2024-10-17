<script lang="ts">
    import { page } from "$app/stores";
    import { onMount, type ComponentType } from "svelte";
    import { goto, invalidateAll } from "$app/navigation";

    import MaterialSymbolsFileCopyOutlineRounded from "~icons/material-symbols/file-copy-outline-rounded";
    import CiSearchMagnifyingGlass from "~icons/ci/search-magnifying-glass";
    import HugeiconsPackageDelivered from "~icons/hugeicons/package-delivered";
    import PajamasRetry from "~icons/pajamas/retry";
    import MaterialSymbolsError from "~icons/material-symbols/error";
    import LineMdChevronDown from "~icons/line-md/chevron-down";

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
    $: error = $page.data.error;
    $: sortBy = $page.url.searchParams.get("sort") || "created_at";
    $: searchBy = $page.url.searchParams.get("search") || "";

    const fields = [
        {
            key: "id",
            label: "",
        },
        {
            key: "file_path",
            label: "Path",
        },
        {
            key: "process_status",
            label: "Status",
        },
        {
            key: "event_source",
            label: "Trigger",
        },
        {
            key: "created_at",
            label: "Created At",
        },
        {
            key: "updated_at",
            label: "Updated At",
        },
    ];

    onMount(() => {
        const interval = setInterval(() => {
            invalidateAll();
        }, 5000);

        return () => clearInterval(interval);
    });

    const updateBasedOn = (key: "search" | "sort", e: Event | string) => {
        const url = new URL(window.location.href);

        let search = "";
        let sort = "";

        if (key === "search" && e instanceof Event) {
            const val = (e.target as HTMLInputElement).value;
            search = val;
        } else {
            search = searchBy;
        }

        if (key === "sort" && typeof e === "string") {
            sort = e;
        } else {
            sort = sortBy;
        }

        if (search) {
            url.searchParams.set("search", search);
        } else {
            url.searchParams.delete("search");
        }

        if (sort) {
            if (key === "sort") {
                if (sort !== sortBy) {
                    sort = sort.split("-").join("");
                } else {
                    sort = sort.startsWith("-") ? sort.slice(1) : `-${sort}`;
                }
            }

            url.searchParams.set("sort", sort);
        } else {
            url.searchParams.delete("sort");
        }

        goto(url.search || "?", {
            invalidateAll: true,
            keepFocus: true,
            noScroll: true,
        });
    };
</script>

{#if error}
    <div class="alert alert-error mt-4">{error}</div>
{/if}

{#if stats}
    <div class="flex flex-col md:flex-row mt-4">
        {#each Object.entries(stats.stats) as [key, val], idx}
            <div class="stat" class:md:border-l={idx !== 0}>
                <div class="stat-figure text-primary">
                    <svelte:component
                        this={iconMap[key]}
                        class="mt-4 lg:mt-0 inline-block h-8 w-8"
                    />
                </div>
                <div class="stat-title">
                    {key[0].toUpperCase() + key.slice(1)}
                </div>
                <div class="stat-value text-primary">{val}</div>
                <div class="hidden lg:block stat-desc">
                    {descMap[key] || ""}
                </div>
            </div>
        {/each}
    </div>
{/if}

{#if events}
    <div class="flex flex-col md:flex-row mt-4">
        <div class="card bg-base-200 shadow-xl">
            <div class="card-body">
                <h2 class="card-title">
                    Events
                    <div class="flex ml-auto gap-2">
                        <input
                            type="text"
                            class="input input-bordered input-sm"
                            placeholder="Search..."
                            on:input={(e) => updateBasedOn("search", e)}
                        />
                    </div>
                </h2>
                <div class="overflow-x-auto">
                    <table class="table text-left">
                        <thead>
                            <tr>
                                {#each fields as field}
                                    <th>
                                        <button
                                            on:click={() =>
                                                updateBasedOn(
                                                    "sort",
                                                    field.key,
                                                )}
                                            class="flex bg-transparent items-center gap-2"
                                        >
                                            <span>{field.label}</span>
                                            <span
                                                class="transform transition"
                                                class:opacity-0={field.key !=
                                                    sortBy.split("-").join("")}
                                                class:rotate-180={field.key !==
                                                    sortBy}
                                            >
                                                {#if field.key != "id"}
                                                    <LineMdChevronDown
                                                        class="ml-auto w-4 h-4"
                                                    />
                                                {/if}
                                            </span>
                                        </button>
                                    </th>
                                {/each}
                                <th>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {#each events as event}
                                <tr
                                    class="cursor-pointer hover:bg-base-300 rounded-md"
                                    on:click={() => goto(`/status/${event.id}`)}
                                >
                                    {#each fields as field}
                                        {#if field.key === "created_at" || field.key === "updated_at"}
                                            <td>
                                                {new Date(
                                                    event[field.key] + "Z",
                                                ).toLocaleString("en-UK")}
                                            </td>
                                        {:else if field.key === "id"}
                                            <td
                                                >{event[field.key].split(
                                                    "-",
                                                )[0]}</td
                                            >
                                        {:else}
                                            <td>{event[field.key]}</td>
                                        {/if}
                                    {/each}
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
{/if}

<style>
    th,
    td {
        padding: 0.5rem;
    }
</style>
