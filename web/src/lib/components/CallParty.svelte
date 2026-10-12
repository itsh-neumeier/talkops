<script lang="ts">
	import type { CallParty } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	let { party }: { party: CallParty } = $props();
</script>

{#if party.name}<strong class="font-medium">{party.name}</strong>{/if}
{#if party.name}{' '}{/if}
{#if party.number && party.number !== 'anonymous'}
	<span class="inline-block max-w-full break-all text-slate-500 dark:text-slate-400"
		>[{party.number}]</span
	>
{:else}
	<span>{t('dash.party.anonymous')}</span>
{/if}
{#if party.ambiguous}
	<span class="text-xs text-slate-500 dark:text-slate-400">({t('dash.party.multiple')})</span>
{/if}
