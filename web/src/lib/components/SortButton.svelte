<script lang="ts">
	import type { Order } from '#lib/list/sort.ts';
	import { t } from '#lib/i18n/index.svelte.ts';

	/**
	 * Sort button for a table header. Put `aria-sort` on the `th`
	 * (`ariaSort()` in `#lib/list/sort.ts`).
	 */
	let {
		label,
		active,
		order,
		onclick
	}: {
		label: string;
		/** The list is sorted by this column. */
		active: boolean;
		order: Order;
		onclick: () => void;
	} = $props();
</script>

<button
	type="button"
	class="group inline-flex items-center gap-1 rounded-md font-medium transition-colors duration-150 hover:text-slate-900 focus-visible:outline-2 focus-visible:outline-teal-500 motion-reduce:transition-none dark:hover:text-slate-100 {active
		? 'text-slate-900 dark:text-slate-100'
		: ''}"
	title={t('filter.sortBy', { column: label })}
	{onclick}
>
	{label}
	<span
		class="inline-block text-xs transition-transform duration-200 motion-reduce:transition-none {active
			? 'text-teal-600 dark:text-teal-400'
			: 'text-slate-300 group-hover:text-slate-400 dark:text-slate-600'} {active && order === 'asc'
			? 'rotate-180'
			: ''}"
		aria-hidden="true">▼</span
	>
</button>
