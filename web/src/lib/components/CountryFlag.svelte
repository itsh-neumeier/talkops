<script lang="ts" module>
	const flags = import.meta.glob('/node_modules/flag-icons/flags/4x3/*.svg', {
		eager: true,
		query: '?url&no-inline',
		import: 'default'
	}) as Record<string, string>;
</script>

<script lang="ts">
	// Flag and localized name of an ISO country code.
	import { i18n } from '#lib/i18n/index.svelte.ts';

	let { country, name = true }: { country: string; name?: boolean } = $props();

	const source = $derived(flags[`/node_modules/flag-icons/flags/4x3/${country.toLowerCase()}.svg`]);
	const label = $derived(
		new Intl.DisplayNames([i18n.locale], { type: 'region' }).of(country.toUpperCase()) ?? country
	);
</script>

<span class="inline-flex items-center gap-1">
	{#if source}
		<img
			src={source}
			alt={name ? '' : label}
			width="16"
			height="12"
			class="inline-block h-3 w-4 rounded-[1px] ring-1 ring-slate-500/20"
			data-country={country}
		/>
	{/if}
	{#if name}<span>{label}</span>{/if}
</span>
