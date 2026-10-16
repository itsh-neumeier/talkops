<script lang="ts">
	/**
	 * Check boxes (several values) or radio buttons (one value) in a
	 * `ColumnFilter` panel.
	 */
	let {
		options,
		selected,
		onchange,
		single = false
	}: {
		options: readonly { value: string; label: string; icon?: string }[];
		selected: readonly string[];
		onchange: (selected: string[]) => void;
		/** Radio buttons: exactly one value. */
		single?: boolean;
	} = $props();

	const name = $props.id();

	function change(value: string) {
		if (single) onchange([value]);
		else
			onchange(
				selected.includes(value) ? selected.filter((v) => v !== value) : [...selected, value]
			);
	}
</script>

<div class="max-h-72 space-y-1 overflow-y-auto">
	{#each options as o (o.value)}
		<label
			class="flex cursor-pointer items-center gap-2 rounded-md px-1 py-1 hover:bg-slate-50 dark:hover:bg-slate-800"
		>
			<input
				type={single ? 'radio' : 'checkbox'}
				{name}
				class="h-4 w-4 shrink-0 accent-teal-600"
				checked={selected.includes(o.value)}
				onchange={() => change(o.value)}
			/>
			{#if o.icon}<span aria-hidden="true">{o.icon}</span>{/if}
			<span class="min-w-0 [overflow-wrap:anywhere]">{o.label}</span>
		</label>
	{/each}
</div>
