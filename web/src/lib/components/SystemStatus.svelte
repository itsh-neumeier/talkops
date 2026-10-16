<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { api, type LiveStatus, type TrunkDetail } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Skeleton from '#lib/components/Skeleton.svelte';
	import { freeswitchUptime, since, type DiagnosticsView } from '#lib/diagnostics.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let view = $state<DiagnosticsView | null>(null);
	let live = $state<LiveStatus | null>(null);
	let trunks = $state<TrunkDetail[] | null>(null);
	let error = $state('');
	let busy = $state(false);
	let now = $state(Date.now());
	let clock: ReturnType<typeof setInterval> | undefined;

	async function load() {
		busy = true;
		error = '';
		try {
			[view, live, trunks] = await Promise.all([
				api.get<DiagnosticsView>('/diagnostics'),
				can('calls.active') ? api.get<LiveStatus>('/telephony/status') : Promise.resolve(null),
				can('trunks.view') ? api.get<TrunkDetail[]>('/trunks') : Promise.resolve(null)
			]);
			now = Date.now();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	onMount(() => {
		load();
		clock = setInterval(() => (now = Date.now()), 30_000);
	});
	onDestroy(() => clearInterval(clock));

	const fsUptime = $derived(freeswitchUptime(view?.freeswitch));
	const accounts = $derived(
		(trunks ?? []).flatMap((tr) =>
			tr.accounts.map((a) => ({ trunk: tr, account: a, up: a.state?.state === 'REGED' }))
		)
	);
</script>

<ErrorBox {error} />

<section class="card space-y-3">
	<div class="flex flex-wrap items-center justify-between gap-2">
		<h2>{t('diag.overview')}</h2>
		<button class="btn" disabled={busy} onclick={load}>↻ {t('diag.refresh')}</button>
	</div>
	{#if view}
		<dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-sm">
			<dt class="text-slate-500 dark:text-slate-400">{t('diag.version')}</dt>
			<dd class="font-mono">{view.version}</dd>
			<dt class="text-slate-500 dark:text-slate-400">{t('diag.serverUptime')}</dt>
			<dd>{since(view.server_started_at, now)}</dd>
			<dt class="text-slate-500 dark:text-slate-400">FreeSWITCH</dt>
			<dd>
				{#if view.freeswitch}
					<span class="badge badge-ok">{t('diag.connected')}</span>
					<span class="ml-2 text-slate-600 dark:text-slate-300"
						>{t('diag.fsUptime', { uptime: fsUptime })}</span
					>
				{:else}
					<span class="badge badge-warn">{t('diag.disconnected')}</span>
				{/if}
			</dd>
			<dt class="text-slate-500 dark:text-slate-400">{t('diag.activeLegs')}</dt>
			<dd>{view.channels.length}</dd>
			{#if live}
				<dt class="text-slate-500 dark:text-slate-400">{t('dash.registrations')}</dt>
				<dd class="tabular-nums">{live.registrations.length}</dd>
			{/if}
		</dl>
	{:else if !error}
		<Skeleton lines={4} />
	{/if}
</section>

{#if trunks}
	<section class="card space-y-3">
		<h2>{t('diag.trunks')}</h2>
		{#if accounts.length}
			<div class="scroll-x">
				<table class="w-full text-left text-sm">
					<thead class="text-slate-500 dark:text-slate-400">
						<tr>
							<th class="py-1 pr-3">{t('diag.trunk')}</th>
							<th class="py-1 pr-3">{t('diag.account')}</th>
							<th class="py-1 pr-3">{t('diag.chState')}</th>
							<th class="col-opt py-1 pr-3">{t('diag.lastError')}</th>
						</tr>
					</thead>
					<tbody>
						{#each accounts as a (a.account.id)}
							<tr class="border-t border-slate-200 align-top dark:border-slate-700">
								<td class="py-1 pr-3">
									<a
										class="text-teal-700 hover:underline dark:text-teal-300"
										href="/trunks/{a.trunk.id}">{a.trunk.name}</a
									>
								</td>
								<td class="py-1 pr-3 font-mono text-xs break-all">{a.account.username}</td>
								<td class="py-1 pr-3">
									{#if !a.trunk.enabled || !a.account.enabled}
										<span class="badge badge-muted">{t('diag.trunkDisabled')}</span>
									{:else if a.up}
										<span class="badge badge-ok">{t('diag.registered')}</span>
									{:else}
										<span class="badge badge-bad">{a.account.state?.state ?? '—'}</span>
									{/if}
								</td>
								<td class="col-opt py-1 pr-3 text-xs break-all"
									>{a.account.state?.last_error ?? ''}</td
								>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else}
			<p class="text-sm text-slate-500 dark:text-slate-400">{t('diag.noTrunks')}</p>
		{/if}
	</section>
{/if}

{#if live}
	<section class="card space-y-3">
		<h2>{t('diag.registrations')}</h2>
		{#if live.registrations.length}
			<div class="scroll-x">
				<table class="w-full text-left text-sm">
					<thead class="text-slate-500 dark:text-slate-400">
						<tr>
							<th class="py-1 pr-3">{t('diag.regUser')}</th>
							<th class="py-1 pr-3">{t('diag.regAddress')}</th>
							<th class="col-opt py-1 pr-3">{t('diag.regTransport')}</th>
							<th class="col-opt-lg py-1 pr-3">{t('diag.regAgent')}</th>
						</tr>
					</thead>
					<tbody>
						{#each live.registrations as r, i (`${r.user}-${r.network_ip}-${r.network_port}-${i}`)}
							<tr class="border-t border-slate-200 align-top dark:border-slate-700">
								<td class="py-1 pr-3 font-mono text-xs break-all">{r.user}</td>
								<td class="py-1 pr-3 font-mono text-xs">{r.network_ip}:{r.network_port}</td>
								<td class="col-opt py-1 pr-3 text-xs">{r.transport}</td>
								<td class="col-opt-lg py-1 pr-3 text-xs break-all">{r.user_agent}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else}
			<p class="text-sm text-slate-500 dark:text-slate-400">{t('diag.noRegistrations')}</p>
		{/if}
	</section>
{/if}

<section class="card space-y-3">
	<div class="flex flex-wrap items-center justify-between gap-2">
		<h2>{t('diag.channels')}</h2>
		<button class="btn" disabled={busy} onclick={load}>↻ {t('diag.refresh')}</button>
	</div>
	{#if view?.channels.length}
		<div class="scroll-x">
			<table class="w-full text-left text-sm">
				<thead class="text-slate-500 dark:text-slate-400">
					<tr>
						<th class="py-1 pr-3">{t('diag.chName')}</th>
						<th class="py-1 pr-3">{t('diag.chCaller')}</th>
						<th class="py-1 pr-3">{t('diag.chDest')}</th>
						<th class="py-1 pr-3">{t('diag.chState')}</th>
						<th class="col-opt py-1 pr-3">Codec</th>
						<th class="col-opt-lg py-1 pr-3">{t('diag.chApp')}</th>
					</tr>
				</thead>
				<tbody>
					{#each view.channels as c (c.uuid)}
						<tr class="border-t border-slate-200 align-top dark:border-slate-700">
							<td class="py-1 pr-3 font-mono text-xs break-all">{c.name}</td>
							<td class="py-1 pr-3">{c.cid_name} <span class="font-mono">{c.cid_num}</span></td>
							<td class="py-1 pr-3 font-mono">{c.dest}</td>
							<td class="py-1 pr-3">{c.callstate || c.state}</td>
							<td class="col-opt py-1 pr-3 font-mono text-xs"
								>{c.read_codec}{c.secure ? ` · ${c.secure}` : ''}</td
							>
							<td class="col-opt-lg py-1 pr-3 font-mono text-xs break-all"
								>{c.application} {c.application_data}</td
							>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{:else if view}
		<p class="text-sm text-slate-500 dark:text-slate-400">{t('diag.noChannels')}</p>
	{/if}
</section>
