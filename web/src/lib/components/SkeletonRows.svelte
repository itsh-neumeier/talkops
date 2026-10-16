<script lang="ts">
	/** Placeholder table rows while a list loads (visible from the first frame). */
	let { rows = 4, cols = 3 }: { rows?: number; cols?: number } = $props();
</script>

{#each { length: rows } as _, r (r)}
	<tr class="skeleton" aria-busy="true" data-testid="skeleton">
		{#each { length: cols } as _, c (c)}
			<td><div class="skeleton-bar h-4 {c === 0 ? 'w-16' : 'w-3/4'}"></div></td>
		{/each}
	</tr>
{/each}
