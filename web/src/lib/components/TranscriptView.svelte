<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type Transcript, type TranscriptStatus } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';

	let { url, status }: { url: string; status: TranscriptStatus } = $props();

	let transcript = $state<Transcript | null>(null);
	let failed = $state(false);

	onMount(async () => {
		if (status !== 'done') return;
		try {
			transcript = await api.get<Transcript>(url);
		} catch {
			failed = true;
		}
	});

	const time = (s: number) =>
		`${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`;
</script>

<div class="text-sm">
	{#if status === 'pending'}
		<p class="text-slate-500">{t('rec.transcriptPending')}</p>
	{:else if status === 'failed' || failed}
		<p class="text-slate-500">{t('rec.transcriptFailed')}</p>
	{:else if transcript && transcript.segments.length === 0}
		<p class="text-slate-500">{t('rec.transcriptEmpty')}</p>
	{:else if transcript}
		<ul class="space-y-1">
			{#each transcript.segments as seg, i (i)}
				<li class="flex gap-2">
					<span class="w-10 shrink-0 font-mono text-xs leading-5 text-slate-400"
						>{time(seg.start)}</span
					>
					{#if seg.speaker}
						<span
							class="w-20 shrink-0 text-xs leading-5 font-medium {seg.speaker === 'caller'
								? 'text-sky-700 dark:text-sky-400'
								: 'text-emerald-700 dark:text-emerald-400'}"
							>{t(seg.speaker === 'caller' ? 'rec.speaker.caller' : 'rec.speaker.called')}</span
						>
					{/if}
					<span>{seg.text}</span>
				</li>
			{/each}
		</ul>
		<p class="hint mt-2">{t('rec.transcriptHint')}</p>
	{/if}
</div>
