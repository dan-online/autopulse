<script lang="ts">
import "@unocss/reset/tailwind-compat.css";
import "virtual:uno.css";

import { page } from "$app/stores";
import Navbar from "$lib/components/core/Navbar.svelte";

$: colorMode = $page.data.colorMode;
</script>

<svelte:head>
    <title>autopulse</title>
    <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png">
    <link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png">
    <link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png">
</svelte:head>

<div
    data-theme={colorMode === "dark" ? "forest" : "winter"}
    class="font-base text-sm min-h-screen"
>
    <Navbar />
    <div class="mx-auto max-w-7xl px-2 sm:px-6 lg:px-8 pb-10">
        <slot />
    </div>
</div>

<style>
    :root {
        font-family: "Inter", sans-serif;
    }
</style>
