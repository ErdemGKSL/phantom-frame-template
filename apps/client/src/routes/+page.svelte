<script lang="ts">
    import type { PageData } from "./$types";
    import { apiFetch } from "$lib/api";

    let { data }: { data: PageData } = $props();

    // svelte-ignore state_referenced_locally
    let counter = $state(data.counter);

    async function increment() {
        const res = await apiFetch<{ value: number }>("/api/increment");
        counter = res.value;
    }
</script>

<h1>Welcome to SvelteKit</h1>
<p>Visit <a href="https://svelte.dev/docs/kit">svelte.dev/docs/kit</a> to read the documentation</p>
<p>Counter: {counter}</p>
<button onclick={increment}>Increment</button>

<style>
    h1 {
        font-size: 2em;
        color: red;
    }

    button {
        margin-top: 1rem;
        padding: 0.5rem 1.25rem;
        font-size: 1rem;
        cursor: pointer;
    }
</style>
