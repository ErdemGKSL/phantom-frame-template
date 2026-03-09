import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

// The adapter is selected at build time via the SVELTE_ADAPTER environment
// variable, which is set by the Rust build.rs script based on the active
// Cargo feature ('bun' or 'node').  When running `bun run build` directly
// (outside of Cargo) the variable defaults to 'bun' so the project works
// without any extra configuration.
const adapterName = process.env.SVELTE_ADAPTER ?? 'node';

import { default as nodeAdapter } from '@sveltejs/adapter-node';
import { default as bunAdapter } from 'svelte-adapter-bun';

const adapter = adapterName === 'node' ? nodeAdapter() : bunAdapter();

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: { adapter }
};

export default config;
