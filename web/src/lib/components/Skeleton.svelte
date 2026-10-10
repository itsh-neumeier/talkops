<script lang="ts">
	/** Placeholder bars while content loads. */
	let { lines = 3, class: extra = '' }: { lines?: number; class?: string } = $props();
	const widths = ['w-3/4', 'w-full', 'w-5/6', 'w-2/3', 'w-11/12'];
</script>

<div
	class="animate-pulse space-y-3 {extra}"
	aria-busy="true"
	aria-live="polite"
	data-testid="skeleton"
>
	{#each { length: lines } as _, i (i)}
		<div class="h-4 rounded bg-slate-200 dark:bg-slate-700 {widths[i % widths.length]}"></div>
	{/each}
</div>
