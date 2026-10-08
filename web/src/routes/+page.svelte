<script lang="ts">
	import { onMount } from 'svelte';
	import {
		api,
		fetchStatus,
		type ActiveCall,
		type Call,
		type CallStats,
		type LiveStatus,
		type StatsRange,
		type SystemStatus,
		type TrunkDetail
	} from '#lib/api.ts';
	import CallChart from '#lib/components/CallChart.svelte';
	import GettingStarted from '#lib/components/GettingStarted.svelte';
	import { formatDateTime, formatDuration, t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';

	const ranges: { id: StatsRange; key: MessageKey }[] = [
		{ id: '1h', key: 'dash.range.1h' },
		{ id: '1d', key: 'dash.range.1d' },
		{ id: '1w', key: 'dash.range.1w' },
		{ id: '1m', key: 'dash.range.1m' }
	];

	let status = $state<SystemStatus | null>(null);
	let live = $state<LiveStatus | null>(null);
	let trunks = $state<TrunkDetail[]>([]);
	let stats = $state<CallStats | null>(null);
	let active = $state<ActiveCall[]>([]);
	let recent = $state<Call[]>([]);
	let lastBackup = $state<string | null>(null);
	let range = $state<StatsRange>('1d');
	let failed = $state(false);
	let now = $state(Date.now());

	async function refresh() {
		try {
			status = await fetchStatus();
			recent = await api.get<Call[]>('/calls?limit=8');
			if (hasRole('operator')) {
				[live, trunks, stats, active] = await Promise.all([
					api.get<LiveStatus>('/telephony/status'),
					api.get<TrunkDetail[]>('/trunks'),
					api.get<CallStats>(`/stats/calls?range=${range}`),
					api.get<ActiveCall[]>('/telephony/calls')
				]);
			}
			failed = false;
		} catch {
			failed = true;
		}
	}

	async function loadOnce() {
		if (!hasRole('admin')) return;
		try {
			const b = await api.get<{ settings: { last_run_at: string | null } }>('/backups');
			lastBackup = b.settings.last_run_at;
		} catch {
			// backups not configured
		}
	}

	async function setRange(r: StatsRange) {
		range = r;
		try {
			stats = await api.get<CallStats>(`/stats/calls?range=${r}`);
		} catch {
			// shown on the next refresh
		}
	}

	onMount(() => {
		refresh();
		loadOnce();
		const timer = setInterval(refresh, 10_000);
		const clock = setInterval(() => (now = Date.now()), 1000);
		return () => {
			clearInterval(timer);
			clearInterval(clock);
		};
	});

	function registered(trunk: TrunkDetail) {
		return trunk.accounts.filter((a) => a.state?.state === 'REGED').length;
	}
	const trunksUp = $derived(
		trunks.filter(
			(tr) => tr.enabled && tr.accounts.length > 0 && registered(tr) === tr.accounts.length
		).length
	);
	const since = (iso: string | null) =>
		iso ? formatDuration(Math.max(0, Math.round((now - Date.parse(iso)) / 1000))) : '';
	const missed = (c: Call) => c.direction === 'inbound' && !c.answered_at;

	const DIRECTION_ICON: Record<Call['direction'], string> = {
		inbound: 'M17 7 7 17M7 9v8h8',
		outbound: 'M7 17 17 7M9 7h8v8',
		internal: 'M4 12h16M14 6l6 6-6 6'
	};
</script>

<div class="space-y-4">
	<h1>{t('nav.dashboard')}</h1>
	{#if hasRole('admin')}<GettingStarted
			registrations={live ? live.registrations.length : null}
		/>{/if}
	{#if failed}
		<p class="card text-red-600 dark:text-red-400">{t('status.error')}</p>
	{/if}

	{#if hasRole('operator') && stats}
		<div class="grid grid-cols-2 gap-3 lg:grid-cols-5">
			{#each [['dash.kpi.active', String(active.filter((c) => c.state !== 'ringing').length)], ['dash.kpi.calls', String(stats.total)], ['dash.kpi.missed', String(stats.missed)], ['dash.kpi.answerRate', stats.answer_rate === null ? '—' : `${stats.answer_rate.toLocaleString()} %`], ['dash.kpi.avgTalk', formatDuration(stats.avg_talk_secs)]] as const as [key, value] (key)}
				<div class="card">
					<p class="text-xs text-slate-500 dark:text-slate-400">{t(key)}</p>
					<p
						class="mt-1 text-2xl font-semibold tabular-nums {key === 'dash.kpi.missed' &&
						stats.missed > 0
							? 'text-red-600 dark:text-red-400'
							: ''}"
					>
						{value}
					</p>
				</div>
			{/each}
		</div>

		<div class="grid gap-4 lg:grid-cols-3">
			<section class="card space-y-3 lg:col-span-2">
				<div class="flex flex-wrap items-center justify-between gap-2">
					<h2>{t('dash.chart.title')}</h2>
					<div class="inline-flex rounded-lg border border-slate-300 p-0.5 dark:border-slate-600">
						{#each ranges as r (r.id)}
							<button
								class="rounded-md px-2.5 py-0.5 text-sm {range === r.id
									? 'bg-teal-600 text-white'
									: 'text-slate-600 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-700'}"
								onclick={() => setRange(r.id)}>{t(r.key)}</button
							>
						{/each}
					</div>
				</div>
				<CallChart {stats} />
				<p class="text-xs text-slate-500">
					{t('dash.chart.summary', {
						inbound: stats.inbound,
						outbound: stats.outbound,
						internal: stats.internal
					})}
				</p>
			</section>

			<section class="card space-y-3">
				<h2>{t('status.title')}</h2>
				{#if status}
					<dl class="divide-y divide-slate-100 text-sm dark:divide-slate-800">
						<div class="flex justify-between py-1.5">
							<dt class="text-slate-500">{t('status.version')}</dt>
							<dd class="font-mono">{status.version}</dd>
						</div>
						{#each [['status.database', status.database], ['status.freeswitch', status.freeswitch]] as const as [label, component] (label)}
							<div class="flex justify-between py-1.5">
								<dt class="text-slate-500">{t(label)}</dt>
								<dd>
									<span class="badge {component.ok ? 'badge-ok' : 'badge-bad'}"
										>{component.ok ? t('status.ok') : t('status.down')}</span
									>
								</dd>
							</div>
						{/each}
						{#if live}
							<div class="flex justify-between py-1.5">
								<dt class="text-slate-500">{t('dash.registrations')}</dt>
								<dd class="tabular-nums">
									{live.registrations.length}
								</dd>
							</div>
						{/if}
						<div class="flex justify-between py-1.5">
							<dt class="text-slate-500">{t('dash.trunks')}</dt>
							<dd>
								{#if trunks.length === 0}<a
										class="text-teal-700 hover:underline dark:text-teal-300"
										href="/trunks">{t('dash.addTrunk')}</a
									>{:else}<span
										class="badge {trunksUp === trunks.filter((tr) => tr.enabled).length
											? 'badge-ok'
											: 'badge-bad'}">{trunksUp}/{trunks.filter((tr) => tr.enabled).length}</span
									>{/if}
							</dd>
						</div>
						{#if hasRole('admin')}
							<div class="flex justify-between py-1.5">
								<dt class="text-slate-500">{t('dash.lastBackup')}</dt>
								<dd>{lastBackup ? formatDateTime(lastBackup) : '—'}</dd>
							</div>
						{/if}
					</dl>
				{/if}
				{#if trunks.length > 0}
					<ul class="space-y-1 text-sm">
						{#each trunks as trunk (trunk.id)}
							<li class="flex items-center justify-between">
								<a class="hover:underline" href="/trunks/{trunk.id}">{trunk.name}</a>
								{#if !trunk.enabled}
									<span class="badge badge-muted">{t('common.disabled')}</span>
								{:else}
									<span
										class="badge {registered(trunk) === trunk.accounts.length &&
										trunk.accounts.length > 0
											? 'badge-ok'
											: 'badge-bad'}"
										>{registered(trunk)}/{trunk.accounts.length} {t('trunks.registered')}</span
									>
								{/if}
							</li>
						{/each}
					</ul>
				{/if}
			</section>
		</div>
	{:else if status}
		<section class="card">
			<h2 class="mb-3">{t('status.title')}</h2>
			<p class="text-sm">
				{t('status.version')} <span class="font-mono">{status.version}</span> ·
				<span class="badge {status.freeswitch.ok ? 'badge-ok' : 'badge-bad'}"
					>{status.freeswitch.ok ? t('status.ok') : t('status.down')}</span
				>
			</p>
		</section>
	{/if}

	<div class="grid gap-4 {hasRole('operator') ? 'lg:grid-cols-2' : ''}">
		{#if hasRole('operator')}
			<section class="card space-y-2">
				<h2>{t('dash.active')}</h2>
				{#if active.length === 0}
					<p class="text-sm text-slate-500">{t('dash.noActive')}</p>
				{:else}
					<ul class="divide-y divide-slate-100 dark:divide-slate-800">
						{#each active as c (c.uuid)}
							<li class="flex items-center justify-between gap-3 py-2 text-sm">
								<span class="min-w-0">
									<span class="block truncate">
										<span class="font-mono">{c.caller_number}</span>
										{c.caller_name && c.caller_name !== c.caller_number ? c.caller_name : ''}
										→ <span class="font-mono">{c.callee_number || c.destination}</span>
										{c.callee_name}
									</span>
									<span class="text-xs text-slate-500"
										>{t(`dash.state.${c.state}` as MessageKey)}</span
									>
								</span>
								<span class="flex items-center gap-2">
									{#if c.state === 'talking'}<span
											class="h-2 w-2 animate-pulse rounded-full bg-emerald-500"
										></span>{:else if c.state === 'ringing'}<span
											class="h-2 w-2 animate-pulse rounded-full bg-amber-500"
										></span>{/if}
									<span class="font-mono text-xs tabular-nums">{since(c.started_at)}</span>
								</span>
							</li>
						{/each}
					</ul>
				{/if}
			</section>
		{/if}
		<section class="card space-y-2">
			<div class="flex items-center justify-between">
				<h2>{t('dash.recent')}</h2>
				<a class="text-sm text-teal-700 hover:underline dark:text-teal-300" href="/calls"
					>{t('dash.allCalls')}</a
				>
			</div>
			{#if recent.length === 0}
				<p class="text-sm text-slate-500">{t('dash.noRecent')}</p>
			{:else}
				<ul class="divide-y divide-slate-100 dark:divide-slate-800">
					{#each recent as c (c.id)}
						<li class="flex items-center justify-between gap-3 py-2 text-sm">
							<span class="flex min-w-0 items-center gap-2">
								<svg
									viewBox="0 0 24 24"
									class="h-4 w-4 shrink-0 {missed(c) ? 'text-red-600' : 'text-slate-400'}"
									fill="none"
									stroke="currentColor"
									stroke-width="2"
									stroke-linecap="round"
									stroke-linejoin="round"
									aria-label={t(`dash.dir.${c.direction}` as MessageKey)}
									><path d={DIRECTION_ICON[c.direction]} /></svg
								>
								<span class="min-w-0 truncate {missed(c) ? 'text-red-600 dark:text-red-400' : ''}">
									<span class="font-mono">{c.caller_number || '—'}</span>
									{c.caller_name && c.caller_name !== c.caller_number ? c.caller_name : ''}
									→ <span class="font-mono">{c.destination}</span>
								</span>
							</span>
							<span class="shrink-0 text-right text-xs text-slate-500">
								{formatDateTime(c.started_at)}<br />
								{missed(c) ? t('dash.missedCall') : formatDuration(c.billsec)}
							</span>
						</li>
					{/each}
				</ul>
			{/if}
		</section>
	</div>
</div>
