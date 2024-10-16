<script setup lang="ts">
    import { page } from "$app/stores";

    $: ev = $page.data.ev;
</script>

<div>
    {JSON.stringify(ev)}
</div>
