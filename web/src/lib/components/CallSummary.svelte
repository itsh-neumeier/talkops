<script lang="ts">
	import type { Call, CallParty as Party } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import CallParty from './CallParty.svelte';
	let {
		direction,
		caller,
		called,
		via = null,
		blocked = false
	}: {
		direction: Call['direction'] | null;
		caller: Party;
		called: Party;
		/** Own number: the dialed number (inbound) or the caller ID (outbound). */
		via?: string | null;
		blocked?: boolean;
	} = $props();
</script>

{#snippet viaNumber()}
	{#if via}
		{' '}<span class="text-slate-500 dark:text-slate-400">{t('calls.via')}</span>
		<span class="font-mono whitespace-nowrap">{via}</span>
	{/if}
{/snippet}

{#if direction === 'inbound'}
	{#if blocked}<p>
			{t('block.calledNumber')} <span class="font-mono">{via || called.number || '—'}</span>
		</p>
	{:else}<p>{t('dash.call.inbound')} <CallParty party={called} />{@render viaNumber()}</p>{/if}
	<p>{t('dash.call.from')} <CallParty party={caller} flag={direction === 'inbound'} /></p>
{:else if direction === 'outbound'}
	<p>{t('dash.call.outbound')} <CallParty party={caller} />{@render viaNumber()}</p>
	<p>
		<span aria-hidden="true">→</span><span class="sr-only">{t('dash.call.to')}</span>
		<CallParty party={called} flag={direction === 'outbound'} />
	</p>
{:else}
	<p>
		{t(direction === 'internal' ? 'dash.call.internal' : 'dash.call.unknown')}
		<CallParty party={caller} />
	</p>
	<p>
		<span aria-hidden="true">→</span><span class="sr-only">{t('dash.call.to')}</span>
		<CallParty party={called} />
	</p>
{/if}
