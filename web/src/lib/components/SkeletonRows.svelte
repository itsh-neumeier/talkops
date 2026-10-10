<script lang="ts">
	/** Placeholder table rows while a list loads. */
	let { rows = 4, cols = 3 }: { rows?: number; cols?: number } = $props();
</script>

{#each { length: rows } as _, r (r)}
	<tr class="animate-pulse" aria-busy="true" data-testid="skeleton">
		{#each { length: cols } as _, c (c)}
			<td
				><div
					class="h-4 rounded bg-slate-200 dark:bg-slate-700 {c === 0 ? 'w-16' : 'w-3/4'}"
				></div></td
			>
		{/each}
	</tr>
{/each}
