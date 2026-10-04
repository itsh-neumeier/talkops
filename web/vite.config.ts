import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

// During `npm run dev` API calls are proxied to a locally running `talkops serve`.
const backend = process.env.TALKOPS_DEV_BACKEND ?? 'http://127.0.0.1:8080';

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			// Built as a single-page app and served by the Rust server, which falls
			// back to index.html for unknown paths.
			adapter: adapter({ pages: 'build', assets: 'build', fallback: 'index.html', strict: true })
		})
	],
	server: {
		proxy: {
			'/api': backend,
			'/healthz': backend,
			'/readyz': backend
		}
	}
});
