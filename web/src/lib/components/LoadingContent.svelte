<script lang="ts">
	import { untrack, type Snippet } from 'svelte';
	import { motion } from '#lib/motion.ts';
	let {
		pending,
		placeholder,
		children
	}: { pending: boolean; placeholder: Snippet; children: Snippet } = $props();
	let shown = $state(untrack(() => pending));
	let started = performance.now();
	$effect(() => {
		const loading = pending;
		return untrack(() => {
			if (loading) {
				if (!shown) started = performance.now();
				shown = true;
				return;
			}
			if (!shown) return;
			const remaining = Math.max(0, motion(300) - (performance.now() - started));
			const timer = setTimeout(() => (shown = false), remaining);
			return () => clearTimeout(timer);
		});
	});
</script>

{#if shown}
	{@render placeholder()}
{:else}
	{@render children()}
{/if}
