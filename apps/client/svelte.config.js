import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

// The adapter is selected at build time via the SVELTE_ADAPTER environment
// variable, which is set by the Rust build.rs script based on the active
// Cargo feature ('bun' or 'node').  When running `bun run build` directly
// (outside of Cargo) the variable defaults to 'bun' so the project works
// without any extra configuration.
const adapterName = process.env.SVELTE_ADAPTER ?? 'node';

let adapterFactory;
if (adapterName === 'node') {
	const { default: nodeAdapter } = await import('@sveltejs/adapter-node');
	adapterFactory = nodeAdapter;
} else {
	const { default: bunAdapter } = await import('svelte-adapter-bun');
	adapterFactory = bunAdapter;
}

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: { adapter: adapterFactory() }
};

export default config;
