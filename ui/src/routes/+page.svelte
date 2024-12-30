<script lang="ts">
import { goto, invalidateAll } from "$app/navigation";
import { page } from "$app/stores";
import { type Component, onMount } from "svelte";

import CiSearchMagnifyingGlass from "~icons/ci/search-magnifying-glass";
import HugeiconsPackageDelivered from "~icons/hugeicons/package-delivered";
import HugeiconsQueue01 from "~icons/hugeicons/queue-01";
import LineMdChevronDown from "~icons/line-md/chevron-down";
import MaterialSymbolsError from "~icons/material-symbols/error";
import MaterialSymbolsFileCopyOutlineRounded from "~icons/material-symbols/file-copy-outline-rounded";
import PajamasRetry from "~icons/pajamas/retry";
import PhMagnifyingGlassBold from "~icons/ph/magnifying-glass-bold";
import SvgSpinners90RingWithBg from "~icons/svg-spinners/90-ring-with-bg";

let searchLoading = false;
// if anyone clicks the magnifying glass, let them bypass the search limit
let limiter = true;

type StatNames =
	| "pending"
	| "total"
	| "found"
	| "processed"
	| "retrying"
	| "failed";

const statNames: StatNames[] = [
	"pending",
	"retrying",
	"found",
	"processed",
	"failed",
	"total",
];

function correctSort(a: [string, unknown], b: [string, unknown]) {
	const aIdx = statNames.indexOf(a[0] as StatNames);
	const bIdx = statNames.indexOf(b[0] as StatNames);

	return aIdx - bIdx;
}

const iconMap: Record<StatNames, Component> = {
	pending: HugeiconsQueue01,
	total: MaterialSymbolsFileCopyOutlineRounded,
	found: CiSearchMagnifyingGlass,
	processed: HugeiconsPackageDelivered,
	retrying: PajamasRetry,
	failed: MaterialSymbolsError,
};

const descMap: Record<StatNames, string> = {
	pending: "Waiting in queue",
	retrying: "Failed 1/+ processors",
	found: "Found + Matched Hash",
	processed: "Sent to processors",
	failed: "Failed to process",
	total: "Total scan events",
};

$: stats = $page.data.stats;
$: events = $page.data.events;
$: error = $page.data.error;

$: sortBy = $page.url.searchParams.get("sort") || "created_at";
$: searchBy = $page.url.searchParams.get("search") || "";
$: pageBy = $page.url.searchParams.get("page")
	? Number.parseInt($page.url.searchParams.get("page") as string)
	: 1;
$: limitBy = $page.url.searchParams.get("limit")
	? Number.parseInt($page.url.searchParams.get("limit") as string)
	: 10;
$: statusBy = $page.url.searchParams.get("status") || "";

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

let updateTimeout: number;
let updateUrl: string;

function autoReload() {
	invalidateAll().then(() => {
		updateTimeout = setTimeout(autoReload, 5000);
	});
}

onMount(() => {
	autoReload();

	return () => clearTimeout(updateTimeout);
});

const updateBasedOn = (
	key: "search" | "sort" | "page" | "limit" | "status",
	e: Event | string | number,
) => {
	const url = new URL(window.location.href);

	let search = "";
	let sort = "";
	let page = 1;
	let limit = 10;
	let status = "";

	if (key === "search" && e instanceof Event) {
		const val = (e.target as HTMLInputElement).value;
		search = val;
	} else {
		search = searchBy;
	}

	if (key === "sort") {
		sort = e as string;
	} else {
		sort = sortBy;
	}

	if (key === "page") {
		page = e as number;
	} else {
		page = pageBy;
	}

	if (key === "limit" && e instanceof Event) {
		limit = Number.parseInt((e.target as HTMLInputElement).value);
		page = 1;
	} else {
		limit = limitBy;
	}

	if (key === "status" && e instanceof Event) {
		status = (e.target as HTMLSelectElement).value;
	} else {
		status = statusBy;
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

	if (pageBy) {
		url.searchParams.set("page", page.toString());
	} else {
		url.searchParams.delete("page");
	}

	if (limitBy) {
		url.searchParams.set("limit", limit.toString());
	} else {
		url.searchParams.delete("limit");
	}

	if (status) {
		url.searchParams.set("status", status);
	} else {
		url.searchParams.delete("status");
	}

	searchLoading = true;

	updateUrl = url.search || "?";
	clearTimeout(updateTimeout);

	updateTimeout = setTimeout(
		async () => {
			clearTimeout(updateTimeout);

			await goto(updateUrl, {
				invalidateAll: true,
				keepFocus: true,
				noScroll: true,
			});

			searchLoading = false;

			autoReload();
		},
		limiter ? 500 : 1,
	);
};
</script>

{#if error}
    <div class="alert alert-error mt-4">{error}</div>
{/if}

{#if stats}
    <div class="flex flex-col lg:flex-row mt-4">
        {#each Object.entries(stats.stats).sort(correctSort) as [key, val], idx}
            <div class="stat" class:md:border-l={idx !== 0}>
                <div class="stat-figure text-primary">
                    <svelte:component
                        this={iconMap[key as StatNames]}
                        class="mt-4 lg:mt-0 inline-block h-8 w-8"
                    />
                </div>
                <div class="stat-title">
                    {key[0].toUpperCase() + key.slice(1)}
                </div>
                <div class="stat-value text-primary">{val}</div>
                <div class="hidden lg:block stat-desc">
                    {descMap[key as StatNames] || ""}
                </div>
            </div>
        {/each}
    </div>
{/if}

{#if events}
    <div class="flex flex-col md:flex-row mt-4">
        <div class="card bg-base-200 shadow-xl w-full">
            <div class="card-body">
                <div class="flex md:flex-row flex-col gap-x-2 gap-y-3">
                    <h2 class="card-title">Events</h2>
                    <select
                        on:input={(e) => updateBasedOn("status", e)}
                        class="md:ml-4 select select-bordered select-sm"
                    >
                        <option value="">All</option>
                        <option value="pending">Pending</option>
                        <option value="complete">Processed</option>
                        <option value="retry">Retrying</option>
                        <option value="failed">Failed</option>
                    </select>
                    <div class="flex relative items-center md:ml-auto gap-2">
                        <button
                            title={limiter
                                ? "Disable Limiter"
                                : "Enable Limiter"}
                            on:click={() => {
                                limiter = !limiter;
                            }}
                            class="transition bg-transparent absolute left-3.5 opacity-80 -mt-0.25"
                        >
                            {#if searchLoading}
                                <SvgSpinners90RingWithBg class="w-4 h-4" />
                            {:else}
                                <span class:text-primary={!limiter}>
                                    <PhMagnifyingGlassBold class="w-4 h-4" />
                                </span>
                            {/if}
                        </button>
                        <input
                            type="text"
                            class="input input-bordered pl-10 w-full input-sm"
                            placeholder="Search..."
                            value={searchBy}
                            on:input={(e) => updateBasedOn("search", e)}
                        />
                    </div>
                </div>

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

                <div class="card-actions justify-between">
                    <div>
                        <input
                            type="number"
                            value={limitBy}
                            class="input input-bordered input-sm"
                            max={100}
                            min={1}
                            on:change={(e) => updateBasedOn("limit", e)}
                        />
                    </div>
                    <div class="flex gap-2">
                        <button
                            class="btn btn-sm"
                            disabled={pageBy <= 1}
                            on:click={() => {
                                updateBasedOn("page", Math.max(1, pageBy - 1));
                            }}
                        >
                            Previous
                        </button>
                        <button
                            class="btn btn-sm"
                            disabled={events.length < limitBy}
                            on:click={() => {
                                updateBasedOn("page", pageBy + 1);
                            }}
                        >
                            Next
                        </button>
                    </div>
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
