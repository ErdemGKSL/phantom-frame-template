<script>
    import { browser } from "$app/environment";

    let incr = $state(0);

    if (browser) {
        setInterval(() => {
            incr += 1;
        }, 1000);
    }
</script>


<h1>Welcome to SvelteKit</h1>
<p>Visit <a href="https://svelte.dev/docs/kit">svelte.dev/docs/kit</a> to read the documentation</p>
<p>Counter: {incr}</p>

<style>
    h1 {
        font-size: 2em;
        color: red;
    }
</style>