<script lang="ts">
	import type { ActiveCall } from '#lib/api.ts';
	import { t, formatDuration, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import CallParty from './CallParty.svelte';
	import LiveTranscript from './LiveTranscript.svelte';
	let { calls, now }: { calls: ActiveCall[]; now: number } = $props();
	let selected = $state<ActiveCall | null>(null);
	const duration = (call: ActiveCall) => {
		const start = call.answered_at ?? call.started_at;
		return start ? formatDuration(Math.max(0, Math.floor((now - Date.parse(start)) / 1000))) : '—';
	};
</script>

<section class="card space-y-2">
	<h2>{t('dash.active')}</h2>
	{#if calls.length === 0}
		<p class="text-sm text-slate-500">{t('dash.noActive')}</p>
	{:else}
		<ul class="divide-y divide-slate-100 dark:divide-slate-800">
			{#each calls as call (call.uuid)}
				<li class="flex items-start justify-between gap-3 py-3 text-sm">
					<div class="min-w-0 space-y-1 text-left">
						{#if call.direction === 'inbound'}
							<p>{t('dash.call.inbound')} <CallParty party={call.called} /></p>
							<p>{t('dash.call.from')} <CallParty party={call.caller} /></p>
						{:else if call.direction === 'outbound'}
							<p>{t('dash.call.outbound')} <CallParty party={call.caller} /></p>
							<p>
								<span aria-hidden="true">→</span><span class="sr-only">{t('dash.call.to')}</span>
								<CallParty party={call.called} />
							</p>
						{:else}
							<p>
								{t(call.direction === 'internal' ? 'dash.call.internal' : 'dash.call.unknown')}
								<CallParty party={call.caller} />
							</p>
							<p>
								<span aria-hidden="true">→</span><span class="sr-only">{t('dash.call.to')}</span>
								<CallParty party={call.called} />
							</p>
						{/if}
						{#if call.transcript_available}
							<button
								class="btn btn-sm mt-2"
								onclick={() => (selected = call)}
								aria-haspopup="dialog">{t('dash.live.open')}</button
							>
						{/if}
					</div>
					<div class="shrink-0 space-y-1 text-right text-xs">
						<span
							class="flex items-center justify-end gap-2 {call.state === 'talking'
								? 'text-emerald-600 dark:text-emerald-400'
								: 'text-slate-500 dark:text-slate-400'}"
						>
							<span
								aria-hidden="true"
								class="h-2 w-2 rounded-full {call.state === 'talking'
									? 'bg-emerald-500'
									: call.state === 'ringing'
										? 'bg-amber-500'
										: 'bg-slate-400'}"
							></span>
							{t(`dash.state.${call.state}` as MessageKey)}
						</span>
						<span
							class="block font-mono tabular-nums"
							aria-label={t(call.answered_at ? 'dash.call.talkTime' : 'dash.call.waitTime')}
							>{duration(call)}</span
						>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</section>

{#if selected}
	<LiveTranscript
		call={selected}
		onclose={() => (selected = null)}
		ended={!calls.some((call) => call.uuid === selected?.uuid)}
	/>
{/if}
