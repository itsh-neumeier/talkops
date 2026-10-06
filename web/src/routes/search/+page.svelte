<script lang="ts">
	import { api, type SearchHit } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let query = $state('');
	let hits = $state<SearchHit[] | null>(null);
	let error = $state('');
	let playing = $state<string | null>(null);

	async function search(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		playing = null;
		if (!query.trim()) return;
		try {
			hits = await api.get<SearchHit[]>(`/search?q=${encodeURIComponent(query.trim())}`);
		} catch (err) {
			error = errorMessage(err);
		}
	}

	/** Splits a snippet into plain and matched (`[…]`) parts. */
	function parts(snippet: string): { text: string; match: boolean }[] {
		return snippet
			.split(/(\[[^\]]*\])/)
			.filter(Boolean)
			.map((p) =>
				p.startsWith('[') && p.endsWith(']')
					? { text: p.slice(1, -1), match: true }
					: { text: p, match: false }
			);
	}

	const audio = (h: SearchHit) =>
		h.recording_id
			? `/api/v1/recordings/${h.recording_id}/audio`
			: `/api/v1/voicemail/messages/${h.voicemail_id}/audio`;
</script>

<div class="space-y-4">
	<h1>{t('nav.search')}</h1>
	<form class="flex gap-2" onsubmit={search}>
		<input
			class="input mt-0 flex-1"
			type="search"
			bind:value={query}
			placeholder={t('search.placeholder')}
			aria-label={t('nav.search')}
		/>
		<button class="btn btn-primary">{t('common.search')}</button>
	</form>
	<p class="hint">{t('search.hint')}</p>
	<ErrorBox {error} />
	{#if hits && hits.length === 0}
		<p class="text-sm text-slate-500">{t('search.empty')}</p>
	{/if}
	<ul class="space-y-3">
		{#each hits ?? [] as h (h.transcript_id)}
			<li class="card space-y-2">
				<div class="flex flex-wrap items-center justify-between gap-2 text-sm">
					<div>
						<span class="badge">{h.recording_id ? t('rec.recording') : t('nav.voicemail')}</span>
						<span class="font-mono">{h.caller_number || t('vm.unknown')}</span>
						→ <span class="font-mono">{h.destination ?? ''}</span>
					</div>
					<span class="text-slate-500">{formatDateTime(h.created_at)}</span>
				</div>
				<p class="text-sm">
					{#each parts(h.snippet) as p, i (i)}{#if p.match}<mark
								class="rounded bg-amber-200 px-0.5 dark:bg-amber-700/60 dark:text-white"
								>{p.text}</mark
							>{:else}{p.text}{/if}{/each}
				</p>
				{#if playing === h.transcript_id}
					<audio class="w-full" controls autoplay src={audio(h)}></audio>
				{:else}
					<button class="btn btn-sm" onclick={() => (playing = h.transcript_id)}
						>▶ {t('search.play')}</button
					>
				{/if}
			</li>
		{/each}
	</ul>
</div>
