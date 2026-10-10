<script lang="ts">
	import SkeletonRows from '#lib/components/SkeletonRows.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { onMount } from 'svelte';
	import { api, type Call, type Recording } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import TranscriptView from '#lib/components/TranscriptView.svelte';
	import { hasRole } from '#lib/session.svelte.ts';
	import { formatDateTime, formatDuration, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let calls = $state<Call[] | null>(null);
	let search = $state('');
	let error = $state('');
	let open = $state<Recording | null>(null);

	async function toggle(c: Call) {
		if (!c.recording_id || open?.id === c.recording_id) {
			open = null;
			return;
		}
		try {
			open = await api.get<Recording>(`/recordings/${c.recording_id}`);
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove(r: Recording) {
		if (!confirm(t('rec.deleteConfirm'))) return;
		try {
			await api.del(`/recordings/${r.id}`);
			open = null;
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function load() {
		try {
			const q = search.trim() ? `?search=${encodeURIComponent(search.trim())}` : '';
			calls = await api.get<Call[]>(`/calls${q}`);
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	const arrow = { inbound: '↙', outbound: '↗', internal: '↔' } as const;
</script>

<div class="space-y-4">
	<div class="flex flex-wrap items-center justify-between gap-2">
		<h1>{t('nav.calls')}</h1>
		<form
			class="flex gap-2"
			onsubmit={(e) => {
				e.preventDefault();
				load();
			}}
		>
			<input
				class="input mt-0"
				bind:value={search}
				placeholder={t('common.search')}
				aria-label={t('common.search')}
			/>
			<button class="btn">{t('common.search')}</button>
		</form>
	</div>
	<ErrorBox {error} />
	<div class="card overflow-x-auto">
		{#if calls && calls.length === 0}
			<p class="text-sm text-slate-500">{t('calls.empty')}</p>
		{:else}
			<table class="table">
				<thead>
					<tr
						><th>{t('calls.time')}</th><th>{t('calls.direction')}</th><th>{t('calls.from')}</th><th
							>{t('calls.to')}</th
						><th>{t('calls.duration')}</th><th>{t('calls.result')}</th><th
							><span class="sr-only">{t('rec.recording')}</span></th
						></tr
					>
				</thead>
				<tbody>
					{#each calls ?? [] as c (c.id)}
						<tr>
							<td class="whitespace-nowrap">{formatDateTime(c.started_at)}</td>
							<td title={t(`dir.${c.direction}`)}>{arrow[c.direction]} {t(`dir.${c.direction}`)}</td
							>
							<td
								><span class="font-mono">{c.caller_number}</span
								>{#if c.caller_name && c.caller_name !== c.caller_number}<div
										class="text-xs text-slate-500"
									>
										{c.caller_name}
									</div>{/if}</td
							>
							<td class="font-mono">{c.destination}</td>
							<td class="font-mono">{c.answered_at ? formatDuration(c.billsec) : '—'}</td>
							<td>
								{#if c.answered_at}<span class="badge badge-ok">OK</span>
								{:else}<span class="badge badge-bad" title={c.hangup_cause}
										>{t('calls.missed')}</span
									>{/if}
							</td>
							<td>
								{#if c.recording_id}
									<button
										class="btn btn-sm"
										aria-expanded={open?.id === c.recording_id}
										onclick={() => toggle(c)}>▶ {t('rec.recording')}</button
									>
								{/if}
							</td>
						</tr>
						{#if open && open.id === c.recording_id}
							<tr>
								<td colspan="7" class="space-y-3 bg-slate-50 dark:bg-slate-900/40">
									<audio
										class="w-full"
										controls
										preload="none"
										src="/api/v1/recordings/{open.id}/audio"
									></audio>
									<TranscriptView
										url="/recordings/{open.id}/transcript"
										status={open.transcript_status}
									/>
									<div class="flex justify-end gap-1">
										<a class="btn btn-sm" href="/api/v1/recordings/{open.id}/audio" download
											>{t('vm.download')}</a
										>
										{#if hasRole('admin')}
											<button class="btn btn-sm btn-danger" onclick={() => remove(open!)}
												>{t('common.delete')}</button
											>
										{/if}
									</div>
								</td>
							</tr>
						{/if}
					{:else}
						{#if !net.settled}<SkeletonRows cols={7} />{/if}
					{/each}
				</tbody>
			</table>
		{/if}
	</div>
</div>
