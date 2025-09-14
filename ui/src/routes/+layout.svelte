<script lang="ts">
import "@unocss/reset/tailwind-compat.css";
import "virtual:uno.css";

import { asset } from "$app/paths";
import { page } from "$app/state";
import Navbar from "$lib/components/core/Navbar.svelte";

let colorMode = $derived(page.data.colorMode);

let { children } = $props();
</script>

<svelte:head>
    <title>autopulse</title>
    <link
        rel="apple-touch-icon"
        sizes="180x180"
        href={asset("/apple-touch-icon.png")}
    />
    <link
        rel="icon"
        type="image/png"
        sizes="32x32"
        href={asset("/favicon-32x32.png")}
    />
    <link
        rel="icon"
        type="image/png"
        sizes="16x16"
        href={asset("/favicon-16x16.png")}
    />
</svelte:head>

<div
    data-theme={colorMode === "dark" ? "forest" : "winter"}
    class="font-base text-sm min-h-screen"
>
    <Navbar />
    <div class="mx-auto max-w-7xl px-2 sm:px-6 lg:px-8 pb-10">
        <!-- <slot /> -->
        {@render children()}
    </div>
</div>

<style>
    :root {
        font-family: "Inter", sans-serif;
    }
</style>
