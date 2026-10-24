<script lang="ts">
	// Label/value rows of a side panel overview.
	let { rows }: { rows: [string, string][] } = $props();
</script>

<dl class="divide-y divide-slate-100 text-sm dark:divide-slate-800">
	{#each rows as [label, value] (label)}
		<div class="flex justify-between gap-4 py-2">
			<dt class="shrink-0 text-slate-500 dark:text-slate-400">{label}</dt>
			<dd class="min-w-0 text-right break-words">{value}</dd>
		</div>
	{/each}
</dl>
