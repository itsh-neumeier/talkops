<script lang="ts">
	import LoadingContent from './LoadingContent.svelte';
	import Skeleton from './Skeleton.svelte';
	import type { ActiveCall } from '#lib/api.ts';
	import { t, formatDuration, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import CallSummary from './CallSummary.svelte';
	import LiveTranscript from './LiveTranscript.svelte';
	import ExpandToggle from './ExpandToggle.svelte';
	import { readOpen, visibleItems, writeOpen } from '#lib/collapse.ts';
	let {
		calls,
		now,
		loading = false,
		error = false
	}: { calls: ActiveCall[]; now: number; loading?: boolean; error?: boolean } = $props();
	let selected = $state<ActiveCall | null>(null);
	const OPEN_KEY = 'talkops.dash.active.open';
	let open = $state(readOpen(OPEN_KEY));
	// A ringing call stays visible in the collapsed list.
	const shown = $derived(visibleItems(calls, open, undefined, (c) => c.state === 'ringing'));
	const duration = (call: ActiveCall) => {
		const start = call.answered_at ?? call.started_at;
		return start ? formatDuration(Math.max(0, Math.floor((now - Date.parse(start)) / 1000))) : '—';
	};
</script>

<section class="card space-y-2">
	<h2>{t('dash.active')}</h2>
	<LoadingContent pending={loading}>
		{#snippet placeholder()}<Skeleton lines={6} />{/snippet}
		<div class="content-in" data-testid="active-content">
			{#if error}<p role="alert" class="text-sm text-red-600 dark:text-red-400">
					{t('status.error')}
				</p>{/if}
			{#if calls.length === 0 && !error}
				<p class="text-sm text-slate-500">{t('dash.noActive')}</p>
			{:else}
				<ul id="dash-active-list" class="divide-y divide-slate-100 dark:divide-slate-800">
					{#each shown as call (call.uuid)}
						<li class="flex items-start justify-between gap-3 py-3 text-sm">
							<div class="min-w-0 space-y-1 text-left">
								<CallSummary direction={call.direction} caller={call.caller} called={call.called} />
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
				<ExpandToggle
					bind:open
					hidden={calls.length - shown.length}
					controls="dash-active-list"
					ontoggle={(value) => writeOpen(OPEN_KEY, value)}
				/>
			{/if}
		</div>
	</LoadingContent>
</section>

{#if selected}
	<LiveTranscript
		call={selected}
		onclose={() => (selected = null)}
		ended={!calls.some(
			(call) =>
				call.uuid === selected?.uuid &&
				call.called.extension_id === selected.called.extension_id &&
				call.called.number === selected.called.number
		)}
	/>
{/if}
