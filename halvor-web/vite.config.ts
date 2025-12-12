import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	server: {
		watch: {
			usePolling: true
		},
		host: true,
		port: 5173
	},
	build: {
		target: 'esnext'
	},
	optimizeDeps: {
		exclude: ['halvor-wasm']
	}
});

