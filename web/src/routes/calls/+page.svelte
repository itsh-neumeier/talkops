<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type Call } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, formatDuration, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let calls = $state<Call[] | null>(null);
	let search = $state('');
	let error = $state('');

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
						><th>{t('calls.duration')}</th><th>{t('calls.result')}</th></tr
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
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
	</div>
</div>
