<script lang="ts">
    import { onMount, onDestroy } from "svelte";

    export let date: Date;

    let relativeTime = "";

    function updateRelativeTime() {
        const now = new Date();
        const diff = date.getTime() - now.getTime();

        const prefix = diff >= 0 ? "in " : "";
        const suffix = diff < 0 ? " ago" : "";

        const absDiff = Math.abs(diff);

        if (absDiff < 1000) {
            relativeTime = "just now";
        } else if (absDiff < 60000) {
            let seconds = Math.floor(absDiff / 1000);

            relativeTime = `${prefix}${seconds} second${seconds === 1 ? "" : "s"} ${suffix}`;
        } else if (absDiff < 3600000) {
            let minutes = Math.floor(absDiff / 60000);

            relativeTime = `${prefix}${minutes} minute${minutes === 1 ? "" : "s"} ${suffix}`;
        } else if (absDiff < 86400000) {
            let hours = Math.floor(absDiff / 3600000);

            relativeTime = `${prefix}${hours} hour${hours === 1 ? "" : "s"} ${suffix}`;
        } else {
            let days = Math.floor(absDiff / 86400000);

            relativeTime = `${prefix}${days} day${days === 1 ? "" : "s"} ${suffix}`;
        }
    }

    let animationFrame: number;

    function tick() {
        updateRelativeTime();
        animationFrame = requestAnimationFrame(tick);
    }

    onMount(() => {
        tick();

        return () => {
            cancelAnimationFrame(animationFrame);
        };
    });
</script>

<p>{relativeTime}</p>
