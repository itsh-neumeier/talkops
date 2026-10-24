<script lang="ts">
	import PageHeader from '#lib/components/PageHeader.svelte';
	import Skeleton from '#lib/components/Skeleton.svelte';
	import LoadingContent from '#lib/components/LoadingContent.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { onMount } from 'svelte';
	import {
		api,
		fetchStatus,
		type ActiveCall,
		type Call,
		type CallParty as CallPartyData,
		type CallStats,
		type LiveStatus,
		type StatsRange,
		type SystemStatus,
		type TrunkDetail
	} from '#lib/api.ts';
	import CallChart from '#lib/components/CallChart.svelte';
	import ActiveCalls from '#lib/components/ActiveCalls.svelte';
	import CallSummary from '#lib/components/CallSummary.svelte';
	import CallBlockStatus from '#lib/components/CallBlockStatus.svelte';
	import { isBlocked } from '#lib/calls/blocking.ts';
	import GettingStarted from '#lib/components/GettingStarted.svelte';
	import ExpandToggle from '#lib/components/ExpandToggle.svelte';
	import { readOpen, visibleItems, writeOpen } from '#lib/collapse.ts';
	import KpiTile from '#lib/components/KpiTile.svelte';
	import { trend } from '#lib/charts.ts';
	import { formatDateTime, formatDuration, t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { can, isAdmin } from '#lib/session.svelte.ts';

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
	const RECENT_KEY = 'talkops.dash.recent.open';
	let recentOpen = $state(readOpen(RECENT_KEY));
	const recentShown = $derived(visibleItems(recent, recentOpen));
	let recentLoading = $state(true);
	let activeLoading = $state(true);
	let recentError = $state(false);
	let activeError = $state(false);
	let lastBackup = $state<string | null>(null);
	let range = $state<StatsRange>('1d');
	let statusError = $state(false);
	let overviewError = $state(false);
	const failed = $derived(statusError || overviewError);
	let now = $state(Date.now());

	async function refresh() {
		await Promise.all([
			fetchStatus()
				.then((value) => {
					status = value;
					statusError = false;
				})
				.catch(() => {
					statusError = true;
				}),
			api
				.get<Call[]>('/calls?limit=8')
				.then((value) => {
					recent = value;
					recentError = false;
				})
				.catch(() => {
					recentError = true;
				})
				.finally(() => {
					recentLoading = false;
				}),
			...(can('calls.active')
				? [
						api
							.get<ActiveCall[]>('/telephony/calls')
							.then((value) => {
								active = value;
								activeError = false;
							})
							.catch(() => {
								activeError = true;
							})
							.finally(() => {
								activeLoading = false;
							}),
						Promise.all([
							api.get<LiveStatus>('/telephony/status'),
							can('trunks.view') ? api.get<TrunkDetail[]>('/trunks') : Promise.resolve([]),
							api.get<CallStats>(`/stats/calls?range=${range}`)
						])
							.then((values) => {
								[live, trunks, stats] = values;
								overviewError = false;
							})
							.catch(() => {
								overviewError = true;
							})
					]
				: [])
		]);
	}

	async function loadOnce() {
		if (!can('backups.manage')) return;
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
	const missed = (c: Call) => c.direction === 'inbound' && !c.answered_at && !isBlocked(c);
	function markAllowed(number: string) {
		recent = recent.map((call) =>
			call.caller_number === number ? { ...call, allowlisted: true } : call
		);
	}
	/** Fallback when the API sends no resolved party. */
	const rawParty = (number: string, name: string): CallPartyData => ({
		number,
		name: name && name !== number ? name : '',
		extension_id: null,
		ambiguous: false
	});

	const DIRECTION_ICON: Record<Call['direction'], string> = {
		inbound: 'M17 7 7 17M7 9v8h8',
		outbound: 'M7 17 17 7M9 7h8v8',
		internal: 'M4 12h16M14 6l6 6-6 6'
	};
</script>

<div class="space-y-4">
	<PageHeader title={t('nav.dashboard')} />
	{#if isAdmin()}<GettingStarted registrations={live ? live.registrations.length : null} />{/if}
	{#if failed}
		<p class="card text-red-600 dark:text-red-400">{t('status.error')}</p>
	{/if}

	{#if can('calls.active') && stats}
		{@const prev = stats.previous}
		<div class="grid grid-cols-2 gap-3 @3xl:grid-cols-5">
			<KpiTile
				label={t('dash.kpi.active')}
				value={String(active.filter((c) => c.state !== 'ringing').length)}
				accent="text-emerald-600 dark:text-emerald-400"
			/>
			<KpiTile
				label={t('dash.kpi.calls')}
				value={String(stats.total)}
				trend={prev ? trend(stats.total, prev.total) : null}
				spark={stats.buckets.map((b) => b.inbound + b.outbound + b.internal)}
			/>
			<KpiTile
				label={t('dash.kpi.missed')}
				value={String(stats.missed)}
				alert={stats.missed > 0}
				trend={prev ? trend(stats.missed, prev.missed) : null}
				upIsGood={false}
				spark={stats.buckets.map((b) => b.missed)}
				accent="text-red-500 dark:text-red-400"
			/>
			<KpiTile
				label={t('dash.kpi.answerRate')}
				value={stats.answer_rate === null ? '—' : `${stats.answer_rate.toLocaleString()} %`}
				trend={prev ? trend(stats.answer_rate, prev.answer_rate) : null}
				ring={stats.answer_rate}
			/>
			<KpiTile
				label={t('dash.kpi.avgTalk')}
				value={formatDuration(stats.avg_talk_secs)}
				trend={prev && stats.avg_talk_secs > 0
					? trend(stats.avg_talk_secs, prev.avg_talk_secs || null)
					: null}
				accent="text-sky-500 dark:text-sky-400"
			/>
		</div>

		<div class="grid grid-cols-1 gap-4 @3xl:grid-cols-3">
			<section class="card space-y-3 @3xl:col-span-2">
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
						{#if can('backups.manage')}
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
	{:else if can('calls.active') && !net.settled}
		<div class="grid grid-cols-1 gap-4 @3xl:grid-cols-3">
			<div class="card @3xl:col-span-2"><Skeleton lines={8} /></div>
			<div class="card"><Skeleton lines={5} /></div>
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

	<div class="grid grid-cols-1 gap-4 {can('calls.active') ? '@3xl:grid-cols-2' : ''}">
		{#if can('calls.active')}
			<ActiveCalls calls={active} {now} loading={activeLoading} error={activeError} />
		{/if}
		<section class="card space-y-2">
			<div class="flex items-center justify-between">
				<h2>{t('dash.recent')}</h2>
				<a class="text-sm text-teal-700 hover:underline dark:text-teal-300" href="/calls"
					>{t('dash.allCalls')}</a
				>
			</div>
			<LoadingContent pending={recentLoading}>
				{#snippet placeholder()}<Skeleton lines={6} />{/snippet}
				<div class="content-in" data-testid="recent-content">
					{#if recentError}<p role="alert" class="text-sm text-red-600 dark:text-red-400">
							{t('status.error')}
						</p>{/if}
					{#if recent.length === 0 && !recentError}
						<p class="text-sm text-slate-500">{t('dash.noRecent')}</p>
					{:else}
						<ul id="dash-recent-list" class="divide-y divide-slate-100 dark:divide-slate-800">
							{#each recentShown as c (c.id)}
								<li class="flex items-start justify-between gap-3 py-3 text-sm">
									<div class="flex min-w-0 items-start gap-2">
										<svg
											viewBox="0 0 24 24"
											class="mt-0.5 h-4 w-4 shrink-0 {missed(c)
												? 'text-red-600'
												: 'text-slate-400'}"
											fill="none"
											stroke="currentColor"
											stroke-width="2"
											stroke-linecap="round"
											stroke-linejoin="round"
											aria-label={t(`dash.dir.${c.direction}` as MessageKey)}
											><path d={DIRECTION_ICON[c.direction]} /></svg
										>
										<div
											class="min-w-0 space-y-1 text-left {missed(c)
												? 'text-red-600 dark:text-red-400'
												: ''}"
										>
											<CallSummary
												direction={c.direction}
												caller={c.caller ?? rawParty(c.caller_number, c.caller_name)}
												called={c.called ?? rawParty(c.destination, '')}
												via={c.via_number}
												blocked={isBlocked(c)}
											/>
											<CallBlockStatus call={c} onallowed={markAllowed} />
										</div>
									</div>
									<span class="shrink-0 text-right text-xs text-slate-500">
										{formatDateTime(c.started_at)}<br />
										{isBlocked(c)
											? '—'
											: missed(c)
												? t('dash.missedCall')
												: formatDuration(c.billsec)}
									</span>
								</li>
							{/each}
						</ul>
						<ExpandToggle
							bind:open={recentOpen}
							hidden={recent.length - recentShown.length}
							controls="dash-recent-list"
							ontoggle={(value) => writeOpen(RECENT_KEY, value)}
						/>
					{/if}
				</div>
			</LoadingContent>
		</section>
	</div>

	{#await import('#lib/components/CallMapWidget.svelte') then widget}
		<widget.default />
	{/await}
</div>
