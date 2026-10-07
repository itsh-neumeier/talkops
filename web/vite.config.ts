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
			adapter: adapter({ pages: 'build', assets: 'build', fallback: 'index.html', strict: true }),
			// Content Security Policy as <meta> tag; the inline bootstrap script is
			// allowed by its hash. frame-ancestors is sent as header by the server.
			csp: {
				mode: 'hash',
				directives: {
					'default-src': ['self'],
					'script-src': ['self'],
					'style-src': ['self', 'unsafe-inline'],
					'img-src': ['self', 'data:', 'blob:'],
					'media-src': ['self', 'blob:'],
					'connect-src': ['self'],
					'font-src': ['self'],
					'object-src': ['none'],
					'base-uri': ['self'],
					'form-action': ['self']
				}
			}
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
