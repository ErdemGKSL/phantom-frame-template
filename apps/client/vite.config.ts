import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig(({ command }) => ({
	plugins: [tailwindcss(), sveltekit()],
	server: command === 'serve'
		? {
			proxy: {
				'/api': {
					target: 'http://localhost:3000',
					changeOrigin: true,
					secure: false
				}
			}
		}
		: undefined
}));

