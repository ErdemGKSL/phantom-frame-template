import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig(({ command }) => ({
	plugins: [tailwindcss(), sveltekit()],
	server: command === 'serve'
		? {
			proxy: {
				'/api': {
					target: process.env.PUBLIC_RUST_SERVER_PORT ? `http://localhost:${process.env.PUBLIC_RUST_SERVER_PORT}` :'http://localhost:3030',
					changeOrigin: true,
					secure: false
				}
			}
		}
		: undefined
}));

