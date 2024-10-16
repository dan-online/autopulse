<script setup lang="ts">
  import { browser } from "$app/environment";
  import { replaceState } from "$app/navigation";
  import { page } from "$app/stores";
  import icon from "$lib/assets/images/logo-tiny.webp";

  $: path = $page.url.pathname;
  $: colorMode = $page.data.colorMode;

  $: {
    if (browser) {
      if (window.location.search.includes("colorMode")) {
        const url = new URL(window.location.href);

        url.searchParams.delete("colorMode");

        replaceState(url.toString(), $page.state);
      }
    }
  }
</script>

<nav class="bg-base-300">
  <div class="mx-auto max-w-7xl px-2 sm:px-6 lg:px-8">
    <div class="relative flex h-16 items-center justify-between">
      <div class="flex flex-1 gap-6">
        <div class="flex flex-shrink-0 items-center">
          <a href="/">
            <img class="h-8 w-8" src={icon} alt="autopulse" />
          </a>
        </div>
        <div>
          <div class="flex space-x-4 items-center mt-1">
            <a
              href="/"
              class="btn btn-ghost btn-sm"
              class:btn-active={path === "/"}>Dashboard</a
            >
            <a
              href="/add"
              class="btn btn-ghost btn-sm"
              class:btn-active={path === "/add"}>Add</a
            >
          </div>
        </div>
        <div class="ml-auto">
          {#if colorMode === "dark"}
            <a class="btn btn-ghost btn-sm" href="?colorMode=light"> Light </a>
          {:else}
            <a class="btn btn-ghost btn-sm" href="?colorMode=dark"> Dark </a>
          {/if}
        </div>
      </div>
    </div>
  </div>
</nav>
