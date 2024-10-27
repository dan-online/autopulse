<script lang="ts">
import "virtual:uno.css";
import "@unocss/reset/tailwind-compat.css";

import { page } from "$app/stores";
import Navbar from "$lib/components/core/Navbar.svelte";

$: colorMode = $page.data.colorMode;
</script>

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
