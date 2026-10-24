<script lang="ts">
	// "Show N more" / "Show less" below a collapsed list.
	import { t } from '#lib/i18n/index.svelte.ts';

	let {
		open = $bindable(false),
		hidden,
		controls,
		ontoggle
	}: {
		open?: boolean;
		/** Number of entries that the collapsed list hides. */
		hidden: number;
		/** ID of the list element. */
		controls: string;
		ontoggle?: (open: boolean) => void;
	} = $props();
</script>

{#if open || hidden > 0}
	<button
		type="button"
		class="flex w-full items-center justify-center gap-1.5 rounded-lg py-1.5 text-sm font-medium text-teal-700 hover:bg-slate-50 dark:text-teal-300 dark:hover:bg-slate-800"
		aria-expanded={open}
		aria-controls={controls}
		onclick={() => {
			open = !open;
			ontoggle?.(open);
		}}
	>
		{open ? t('dash.showLess') : t('dash.showMore', { count: hidden })}
		<svg
			viewBox="0 0 24 24"
			class="h-4 w-4 transition-transform {open ? 'rotate-180' : ''}"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			stroke-linejoin="round"
			aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg
		>
	</button>
{/if}
