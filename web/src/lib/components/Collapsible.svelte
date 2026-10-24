<script lang="ts">
	// A titled group that opens and closes, like the sections of the UniFi
	// Talk inspector.
	import type { Snippet } from 'svelte';

	let {
		title,
		open = $bindable(true),
		children
	}: { title: string; open?: boolean; children: Snippet } = $props();
	const id = $props.id();
</script>

<section class="rounded-xl border border-slate-200 dark:border-slate-800">
	<h3>
		<button
			type="button"
			class="flex w-full items-center justify-between gap-2 px-3 py-2.5 text-left text-sm font-semibold"
			aria-expanded={open}
			aria-controls="collapsible-{id}"
			onclick={() => (open = !open)}
		>
			{title}
			<svg
				viewBox="0 0 24 24"
				class="h-4 w-4 text-slate-400 transition-transform {open ? 'rotate-180' : ''}"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
				aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg
			>
		</button>
	</h3>
	{#if open}
		<div id="collapsible-{id}" class="border-t border-slate-200 px-3 py-2 dark:border-slate-800">
			{@render children()}
		</div>
	{/if}
</section>
