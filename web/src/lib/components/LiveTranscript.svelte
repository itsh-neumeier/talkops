<script lang="ts">
	import { onMount } from 'svelte';
	import {
		api,
		ApiError,
		type ActiveCall,
		type LiveTranscript,
		type TranscriptSegment
	} from '#lib/api.ts';
	import { t, i18n } from '#lib/i18n/index.svelte.ts';
	import CallParty from './CallParty.svelte';
	import TypedText from './TypedText.svelte';
	let { call, onclose, ended }: { call: ActiveCall; onclose: () => void; ended: boolean } =
		$props();
	let dialog: HTMLDialogElement;
	let preview = $state<LiveTranscript | null>(null);
	let rows = $state<(TranscriptSegment & { key: string; animate: boolean })[]>([]);
	let error = $state(false);
	let stopped = $state(false);
	let busy = false;
	let disposed = false;
	let first = true;
	const path = $derived(`/telephony/calls/${call.uuid}/transcript`);

	async function poll() {
		if (busy || disposed || stopped || ended) return;
		busy = true;
		try {
			const result = await api.post<LiveTranscript>(path);
			if (disposed) {
				void api.del(path).catch(() => {});
				return;
			}
			const previous = new Map(rows.map((row) => [row.key, row]));
			rows = result.segments.map((segment) => {
				const key = `${segment.speaker}:${segment.start}:${segment.end}`;
				return previous.get(key) ?? { ...segment, key, animate: !first };
			});
			preview = result;
			first = false;
			error = result.status === 'failed';
		} catch (e) {
			if (disposed) return;
			if (e instanceof ApiError && e.status === 404) stopped = true;
			else error = true;
		} finally {
			busy = false;
		}
	}

	function time(seconds: number) {
		if (!preview) return '';
		return new Date(Date.parse(preview.started_at) + seconds * 1000).toLocaleTimeString(
			i18n.locale === 'de' ? 'de-DE' : 'en-GB',
			{ hour: '2-digit', minute: '2-digit', second: '2-digit' }
		);
	}

	onMount(() => {
		dialog.showModal();
		void poll();
		const timer = setInterval(poll, 3000);
		return () => {
			disposed = true;
			clearInterval(timer);
			void api.del(path).catch(() => {});
		};
	});
</script>

<dialog
	bind:this={dialog}
	aria-labelledby="live-transcript-title"
	oncancel={onclose}
	{onclose}
	class="m-auto max-h-[90dvh] w-[calc(100%-2rem)] max-w-2xl overflow-y-auto rounded-xl border border-slate-200 bg-white p-5 text-slate-900 shadow-xl backdrop:bg-black/40 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100"
>
	<div class="mb-3 flex items-center justify-between gap-3">
		<h2 id="live-transcript-title">{t('dash.live.title')}</h2>
		<button class="btn btn-sm" onclick={onclose}>{t('common.close')}</button>
	</div>
	<div class="space-y-1 text-sm">
		<p><CallParty party={call.caller} /></p>
		<p>→ <CallParty party={call.called} /></p>
	</div>
	<p class="my-3 text-xs text-slate-500 dark:text-slate-400">{t('dash.live.provisional')}</p>
	{#if error}<p role="alert" class="mb-3 text-sm text-red-600 dark:text-red-400">
			{t('dash.live.error')}
		</p>{/if}
	<p class="mb-3 text-xs text-slate-500 dark:text-slate-400" role="status">
		{ended || stopped
			? t('dash.live.ended')
			: rows.length === 0
				? t('dash.live.waiting')
				: t('dash.live.updating')}
	</p>
	<ol class="space-y-4">
		{#each rows as row (row.key)}
			<li class="border-l-2 border-slate-200 pl-3 dark:border-slate-700">
				<div
					class="mb-1 flex flex-wrap justify-between gap-2 text-xs text-slate-500 dark:text-slate-400"
				>
					<span
						>{t(row.speaker === 'called' ? 'dash.live.called' : 'dash.live.caller')}: {row.speaker ===
						'called'
							? call.called.name || call.called.number
							: call.caller.name || call.caller.number}</span
					>
					<time>{time(row.start)}</time>
				</div>
				<p class="break-words text-sm"><TypedText text={row.text} animate={row.animate} /></p>
			</li>
		{/each}
	</ol>
</dialog>
