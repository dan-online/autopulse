<script lang="ts">
import { browser } from "$app/environment";
import { replaceState } from "$app/navigation";
import { resolve } from "$app/paths";
import { page } from "$app/state";
import icon from "$lib/assets/images/logo-tiny.webp";

let path = $derived(page.url.pathname);
let colorMode = $derived(page.data.colorMode);
let forceAuth = $derived(page.data.forceAuth);

$effect(() => {
	if (browser) {
		if (window.location.search.includes("colorMode")) {
			const url = new URL(window.location.href);

			url.searchParams.delete("colorMode");

			replaceState(url.toString(), page.state);
		}
	}
});
</script>

<nav class="bg-base-300">
  <div class="mx-auto max-w-7xl px-2 sm:px-6 lg:px-8">
    <div class="relative flex h-16 items-center justify-between">
      <div class="flex items-center flex-1 gap-6">
        <div class="flex flex-shrink-0 items-center">
          <a href={resolve("/")}>
            <img class="h-8 w-8" src={icon} alt="autopulse" />
          </a>
        </div>
        <div>
          <div class="flex space-x-4 items-center mt-1">
            <a
              href={resolve("/")}
              class="btn btn-ghost btn-sm"
              class:btn-active={path === resolve("/")}>Dashboard</a
            >
            <a
              href={resolve("/add")}
              class="btn btn-ghost btn-sm"
              class:btn-active={path === resolve("/add")}>Add</a
            >
          </div>
        </div>
        <div class="flex gap-2 items-center pt-1 ml-auto">
          {#if path !== resolve("/login") && !forceAuth}
            <a href={resolve("/login")} class="btn btn-secondary btn-sm" data-sveltekit-preload-data="off">Logout</a>
          {/if}
          {#if colorMode === "dark"}
            <a class="btn btn-ghost btn-circle" href="?colorMode=light" data-sveltekit-preload-data="off">
              <!-- <IcBaselineWbSunny class="w-6 h-6" /> -->
               <i class="w-6 h-6 i-ic-baseline-wb-sunny"></i>
            </a>
          {:else}
            <a class="btn btn-ghost btn-circle" href="?colorMode=dark" data-sveltekit-preload-data="off">
              <!-- <MaterialSymbolsLightNightsStay class="w-6 h-6 -mt-0.25" /> -->
                <i class="w-6 h-6 i-material-symbols-light-nights-stay"></i>
            </a>
          {/if}
        </div>
      </div>
    </div>
  </div>
</nav>
