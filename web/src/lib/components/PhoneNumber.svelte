<script lang="ts" module>
	const flags = import.meta.glob('/node_modules/flag-icons/flags/4x3/*.svg', {
		eager: true,
		query: '?url&no-inline',
		import: 'default'
	}) as Record<string, string>;
</script>

<script lang="ts">
	import { numberCountry } from '#lib/calls/country.ts';
	import { i18n, t } from '#lib/i18n/index.svelte.ts';
	let {
		number,
		countryNumber = number,
		flag = false,
		internal = false,
		brackets = false
	}: {
		number: string;
		countryNumber?: string;
		flag?: boolean;
		internal?: boolean;
		brackets?: boolean;
	} = $props();

	const country = $derived(flag ? numberCountry(countryNumber, internal) : null);
	const source = $derived(
		country ? flags[`/node_modules/flag-icons/flags/4x3/${country.toLowerCase()}.svg`] : null
	);
	const label = $derived(
		country ? (new Intl.DisplayNames([i18n.locale], { type: 'region' }).of(country) ?? country) : ''
	);
</script>

{#if source}
	<img
		src={source}
		alt={t('calls.numberCountry', { country: label })}
		title={t('calls.numberCountry', { country: label })}
		width="16"
		height="12"
		class="mr-1 inline-block h-3 w-4 rounded-[1px] align-baseline ring-1 ring-slate-500/20"
		data-country={country}
	/>
{/if}{brackets ? `[${number}]` : number}
