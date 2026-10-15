<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type TranscriptionJob, type WorkerStatus } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import PageHeader from '#lib/components/PageHeader.svelte';
	import Skeleton from '#lib/components/Skeleton.svelte';
	import Tabs from '#lib/components/Tabs.svelte';
	import { formatDateTime, formatDuration, t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import { formatBytes, formatEta, percent } from '#lib/transcriptions/format.ts';
	import { errorMessage } from '#lib/util.ts';

	let tab = $state<'active' | 'history'>('active');
	let jobs = $state<TranscriptionJob[] | null>(null);
	let history = $state<TranscriptionJob[] | null>(null);
	let workers = $state<WorkerStatus[] | null>(null);
	let error = $state('');
	/** Jobs with an action on the way (buttons wait for the answer). */
	let pending = $state<Record<string, boolean>>({});
	const manage = $derived(can('transcriptions.manage'));

	async function refresh() {
		try {
			const [active, done, w] = await Promise.all([
				api.get<TranscriptionJob[]>('/transcription-jobs'),
				tab === 'history' || history === null
					? api.get<TranscriptionJob[]>('/transcription-jobs?history=true')
					: Promise.resolve(history),
				manage ? api.get<WorkerStatus[]>('/transcription-workers') : Promise.resolve(null)
			]);
			jobs = active;
			history = done;
			workers = w;
			error = '';
		} catch (err) {
			error = errorMessage(err);
		}
	}

	onMount(() => {
		refresh();
		// Every 2 s while the page is visible.
		const timer = setInterval(() => {
			if (!document.hidden) refresh();
		}, 2000);
		return () => clearInterval(timer);
	});

	async function act(job: TranscriptionJob, action: 'pause' | 'resume' | 'stop' | 'prioritize') {
		if (action === 'stop' && !confirm(t('tx.stopConfirm'))) return;
		pending[job.id] = true;
		try {
			await api.post(`/transcription-jobs/${job.id}`, { action });
			await refresh();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			pending[job.id] = false;
		}
	}

	/** Short engine name: the model label without its description. */
	function engineName(key: string): string {
		const [kind, ...rest] = key.split(':');
		if (kind === 'local' && rest[0]) {
			return t(`stt.local.${rest[0]}` as MessageKey).split(' – ')[0];
		}
		return rest.join(':') || '—';
	}

	function source(job: TranscriptionJob): string {
		const party = [job.caller_number, job.destination].filter(Boolean).join(' → ');
		return party || (job.voicemail_id ? t('tx.voicemail') : t('tx.recording'));
	}

	const badge: Record<TranscriptionJob['status'], string> = {
		running: 'badge-ok',
		queued: 'badge-muted',
		paused: 'badge-warn',
		done: 'badge-ok',
		failed: 'badge-bad',
		cancelled: 'badge-muted'
	};

	function share(used: number, total: number): number {
		return total > 0 ? Math.min(1, used / total) : 0;
	}

	function runTime(job: TranscriptionJob): string {
		if (!job.started_at || !job.finished_at) return '—';
		return formatDuration(
			Math.round((Date.parse(job.finished_at) - Date.parse(job.started_at)) / 1000)
		);
	}
</script>

{#snippet meter(label: string, value: number, detail: string)}
	<div>
		<div class="mb-1 flex items-baseline justify-between gap-2 text-xs">
			<span class="font-medium text-slate-600 dark:text-slate-300">{label}</span>
			<span class="text-slate-500 tabular-nums dark:text-slate-400">{detail}</span>
		</div>
		<div
			class="h-2 overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800"
			role="meter"
			aria-label={label}
			aria-valuemin="0"
			aria-valuemax="100"
			aria-valuenow={percent(value)}
		>
			<div
				class="h-full rounded-full transition-[width] duration-700 ease-out motion-reduce:transition-none {value >
				0.85
					? 'bg-red-500'
					: value > 0.6
						? 'bg-amber-500'
						: 'bg-teal-500'}"
				style="width: {percent(value)}%"
			></div>
		</div>
	</div>
{/snippet}

<div class="space-y-4">
	<PageHeader title={t('tx.title')} intro={t('tx.intro')} />
	<ErrorBox {error} />

	{#if manage}
		<section class="card space-y-3" aria-labelledby="tx-resources">
			<h2 id="tx-resources">{t('tx.resources')}</h2>
			{#if workers === null}
				<Skeleton lines={3} />
			{:else if workers.length === 0}
				<p class="text-sm text-slate-500 dark:text-slate-400">{t('tx.noWorkers')}</p>
			{:else}
				<div class="grid grid-cols-1 gap-4 @2xl:grid-cols-2">
					{#each workers as w (w.worker_id)}
						<div class="space-y-3 rounded-lg border border-slate-200 p-3 dark:border-slate-800">
							<div class="flex items-center justify-between gap-2">
								<span class="truncate font-mono text-sm">{w.worker_id}</span>
								<span class="badge {w.job_id ? 'badge-ok' : 'badge-muted'}">
									{#if w.job_id}<span
											class="h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-500 motion-reduce:animate-none"
											aria-hidden="true"
										></span>{/if}
									{w.job_id ? t('tx.busy') : t('tx.idle')}
								</span>
							</div>
							{@render meter(
								t('tx.cpu'),
								share(w.cpu_percent, w.cpus * 100),
								`${Math.round(w.cpu_percent)} % · ${t('tx.cores', { count: w.cpus })}`
							)}
							{@render meter(
								t('tx.memory'),
								share(w.mem_used_bytes, w.mem_total_bytes),
								`${formatBytes(w.mem_used_bytes)} / ${formatBytes(w.mem_total_bytes)}`
							)}
							<dl class="grid grid-cols-2 gap-2 text-xs">
								<div>
									<dt class="text-slate-500 dark:text-slate-400">{t('tx.workerMemory')}</dt>
									<dd class="font-medium tabular-nums">{formatBytes(w.rss_bytes)}</dd>
								</div>
								<div>
									<dt class="text-slate-500 dark:text-slate-400">{t('tx.load')}</dt>
									<dd class="font-medium tabular-nums">{w.load1.toFixed(2)}</dd>
								</div>
							</dl>
						</div>
					{/each}
				</div>
			{/if}
		</section>
	{/if}

	<Tabs
		label={t('tx.title')}
		tabs={[
			{ id: 'active', label: `${t('tx.active')}${jobs?.length ? ` (${jobs.length})` : ''}` },
			{ id: 'history', label: t('tx.history') }
		]}
		bind:value={tab}
		onchange={refresh}
	/>

	{#if tab === 'active'}
		{#if jobs === null}
			<div class="card"><Skeleton lines={4} /></div>
		{:else if jobs.length === 0}
			<div class="card py-10 text-center text-sm text-slate-500 dark:text-slate-400">
				{t('tx.empty')}
			</div>
		{:else}
			<ul class="space-y-3">
				{#each jobs as job (job.id)}
					{@const busy = !!pending[job.id] || !!job.control}
					<li class="card row-in space-y-3">
						<div class="flex flex-wrap items-start justify-between gap-2">
							<div class="min-w-0">
								<div class="flex flex-wrap items-center gap-2">
									<span class="font-medium [overflow-wrap:anywhere]">{source(job)}</span>
									<span class="badge {badge[job.status]}">
										{#if job.status === 'running'}<span
												class="h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-500 motion-reduce:animate-none"
												aria-hidden="true"
											></span>{/if}
										{job.control === 'pause'
											? t('tx.pausing')
											: job.control === 'stop'
												? t('tx.stopping')
												: t(`tx.status.${job.status}`)}
									</span>
								</div>
								<div
									class="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-slate-500 dark:text-slate-400"
								>
									{#if job.call_started_at}<span>{formatDateTime(job.call_started_at)}</span>{/if}
									<span>{t('tx.audio', { time: formatDuration(job.audio_secs) })}</span>
									<span>{engineName(job.engine_key)}</span>
									{#if job.engine}<span>{t('tx.manual')}</span>{/if}
									{#if job.pass >= 2}<span>{t('tx.pass2')}</span>{/if}
									{#if job.requested_by}<span
											>{t('tx.requestedBy', { name: job.requested_by })}</span
										>{/if}
								</div>
							</div>
							<div class="flex flex-wrap gap-1">
								{#if job.status === 'queued'}
									<button class="btn btn-sm" disabled={busy} onclick={() => act(job, 'prioritize')}
										>{t('tx.prioritize')}</button
									>
								{/if}
								{#if job.status === 'running' || job.status === 'queued'}
									<button class="btn btn-sm" disabled={busy} onclick={() => act(job, 'pause')}
										>⏸&#xFE0E; {t('tx.pause')}</button
									>
								{:else if job.status === 'paused'}
									<button
										class="btn btn-sm btn-primary"
										disabled={busy}
										onclick={() => act(job, 'resume')}>▶ {t('tx.resume')}</button
									>
								{/if}
								<button
									class="btn btn-sm btn-danger"
									disabled={busy}
									onclick={() => act(job, 'stop')}>■ {t('tx.stop')}</button
								>
							</div>
						</div>
						{#if job.status !== 'queued' || job.progress}
							<div
								class="h-2.5 overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800"
								role="progressbar"
								aria-label={source(job)}
								aria-valuemin="0"
								aria-valuemax="100"
								aria-valuenow={percent(job.progress)}
							>
								<div
									class="progress-fill h-full rounded-full transition-[width] duration-700 ease-out motion-reduce:transition-none {job.status ===
									'paused'
										? 'bg-amber-400 dark:bg-amber-500'
										: 'progress-running bg-teal-500'}"
									style="width: {percent(job.progress)}%"
								></div>
							</div>
						{/if}
						<div
							class="flex flex-wrap justify-between gap-2 text-xs text-slate-500 dark:text-slate-400"
						>
							<span class="font-medium text-slate-700 tabular-nums dark:text-slate-200"
								>{percent(job.progress)} %</span
							>
							<span class="tabular-nums">
								{#if job.status === 'running'}{t('tx.eta', { time: formatEta(job.eta_secs) })}
								{:else if job.status === 'queued'}{t('tx.etaQueued', {
										time: formatEta(job.eta_secs)
									})}
								{:else if job.resumable}{t('tx.resumable')}{/if}
							</span>
						</div>
					</li>
				{/each}
			</ul>
		{/if}
	{:else if history === null}
		<div class="card"><Skeleton lines={4} /></div>
	{:else if history.length === 0}
		<div class="card py-10 text-center text-sm text-slate-500 dark:text-slate-400">
			{t('tx.emptyHistory')}
		</div>
	{:else}
		<div class="card scroll-x">
			<table class="table">
				<thead>
					<tr>
						<th>{t('calls.time')}</th>
						<th>{t('tx.recording')}</th>
						<th class="col-opt">{t('tx.model')}</th>
						<th class="col-opt">{t('calls.duration')}</th>
						<th>{t('calls.result')}</th>
					</tr>
				</thead>
				<tbody>
					{#each history as job (job.id)}
						<tr>
							<td class="whitespace-nowrap">{formatDateTime(job.finished_at ?? job.created_at)}</td>
							<td class="[overflow-wrap:anywhere]">
								{source(job)}
								{#if job.requested_by}<div class="text-xs text-slate-500 dark:text-slate-400">
										{t('tx.requestedBy', { name: job.requested_by })}
									</div>{/if}
							</td>
							<td class="col-opt">{engineName(job.engine_key)}</td>
							<td class="col-opt font-mono">{runTime(job)}</td>
							<td>
								<span
									class="badge whitespace-nowrap {badge[job.status]}"
									title={job.last_error ?? ''}>{t(`tx.status.${job.status}`)}</span
								>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</div>
