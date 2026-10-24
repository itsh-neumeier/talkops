<script lang="ts">
	// One action in the "Manage" group of a side panel: a text button and an
	// info sign with the explanation (as in UniFi Talk).
	let {
		label,
		hint,
		danger = false,
		href,
		disabled = false,
		onclick
	}: {
		label: string;
		hint: string;
		danger?: boolean;
		href?: string;
		disabled?: boolean;
		onclick?: () => void;
	} = $props();
	const tone = $derived(
		danger
			? 'text-red-600 hover:text-red-700 dark:text-red-400'
			: 'text-teal-700 hover:text-teal-800 dark:text-teal-300'
	);
</script>

<div
	class="flex items-center justify-between gap-3 border-b border-slate-100 py-2.5 last:border-b-0 dark:border-slate-800"
>
	{#if href}
		<a class="text-sm font-medium {tone}" {href}>{label}</a>
	{:else}
		<button
			type="button"
			class="text-left text-sm font-medium disabled:cursor-not-allowed disabled:opacity-50 {tone}"
			{disabled}
			{onclick}>{label}</button
		>
	{/if}
	<span class="group relative shrink-0">
		<svg
			viewBox="0 0 24 24"
			class="h-4 w-4 text-slate-400"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			role="img"
			aria-label={hint}
			><title>{hint}</title><circle cx="12" cy="12" r="9" /><path d="M12 11v5M12 8h.01" /></svg
		>
	</span>
</div>
