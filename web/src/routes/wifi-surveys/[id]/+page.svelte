<script lang="ts">
	// One Wi-Fi survey: the live dashboard while the call runs, the result
	// with roam table, recommendations and playback afterwards.
	import { onDestroy, onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api } from '#lib/api.ts';
	import EmptyRow from '#lib/components/EmptyRow.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import PageHeader from '#lib/components/PageHeader.svelte';
	import WifiSurveyChart from '#lib/components/WifiSurveyChart.svelte';
	import { formatDateTime, t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import {
		ACTIVE,
		apLabel,
		band,
		clock,
		mbit,
		mergeTimeline,
		newestOffset,
		ratingBadge,
		rssiLevel,
		stateBadge,
		type Live,
		type LiveNow,
		type Recommendation,
		type Roam,
		type Summary,
		type Survey,
		type Timeline
	} from '#lib/wifiSurvey.ts';

	const id = $derived(page.params.id as string);

	let survey = $state<Survey | null>(null);
	let roams = $state<Roam[]>([]);
	let timeline = $state<Timeline>({ media: [], radio: [], events: [] });
	let now = $state<LiveNow | null>(null);
	let liveSummary = $state<Summary | null>(null);
	let error = $state('');
	let lastUpdate = $state<Date | null>(null);
	let offline = $state(false);
	let fullChart = $state(false);
	let soundAlert = $state(false);
	let wakeLock: { release: () => Promise<void> } | null = null;
	let wakeLockFailed = $state(false);
	let audio = $state<HTMLAudioElement>();
	let timer: ReturnType<typeof setTimeout> | undefined;
	let stopped = false;
	let noteDraft = $state('');
	let tick = $state(Date.now());

	const active = $derived(!!survey && ACTIVE.includes(survey.state));
	const summary = $derived(survey?.summary ?? liveSummary);

	async function loadDetails() {
		const d = await api.get<{ survey: Survey; roams: Roam[] }>(`/wifi-surveys/${id}`);
		survey = d.survey;
		roams = d.roams;
		if (!ACTIVE.includes(d.survey.state)) {
			timeline = await api.get<Timeline>(`/wifi-surveys/${id}/timeline`);
		}
	}

	async function poll() {
		if (stopped) return;
		try {
			if (!survey || ACTIVE.includes(survey.state)) {
				const live = await api.get<Live>(
					`/wifi-surveys/${id}/live?since=${newestOffset(timeline)}`
				);
				const wasActive = !survey || ACTIVE.includes(survey.state);
				survey = live.survey;
				timeline = mergeTimeline(timeline, live.timeline);
				if (live.timeline.events.length) {
					// Notes can change: take the newest version of each marker.
					const fresh = new Map(live.timeline.events.map((e) => [e.id, e]));
					timeline.events = timeline.events.map((e) => fresh.get(e.id) ?? e);
				}
				const before = now?.rating_5s;
				now = live.now;
				liveSummary = live.analysis.summary;
				roams = live.analysis.roams;
				if (soundAlert && now.rating_5s === 'bad' && before !== 'bad') beep();
				if (wasActive && !ACTIVE.includes(live.survey.state)) await loadDetails();
			} else if (survey.state === 'analysing') {
				await loadDetails();
			}
			lastUpdate = new Date();
			offline = false;
			error = '';
		} catch (err) {
			// A dead zone: the server keeps measuring; values follow later.
			offline = true;
			if (!survey) error = errorMessage(err);
		}
		const next =
			!survey || ACTIVE.includes(survey.state) ? 1000 : survey.state === 'analysing' ? 3000 : 0;
		if (next && !stopped) timer = setTimeout(poll, next);
		else releaseWakeLock();
	}

	async function requestWakeLock() {
		const nav = navigator as Navigator & {
			wakeLock?: { request: (type: 'screen') => Promise<{ release: () => Promise<void> }> };
		};
		if (!nav.wakeLock) {
			wakeLockFailed = true;
			return;
		}
		try {
			wakeLock = await nav.wakeLock.request('screen');
			wakeLockFailed = false;
		} catch {
			wakeLockFailed = true;
		}
	}

	function releaseWakeLock() {
		void wakeLock?.release().catch(() => {});
		wakeLock = null;
	}

	function visibility() {
		if (document.visibilityState === 'visible' && active) requestWakeLock();
	}

	function beep() {
		try {
			const ctx = new AudioContext();
			const osc = ctx.createOscillator();
			osc.frequency.value = 880;
			osc.connect(ctx.destination);
			osc.start();
			osc.stop(ctx.currentTime + 0.15);
		} catch {
			// no audio output
		}
	}

	async function stop() {
		if (!confirm(t('wifi.confirmStop'))) return;
		try {
			await api.post(`/wifi-surveys/${id}/stop`);
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function mark(marker: 'disturbance' | 'location' | 'note') {
		try {
			const ev = await api.post<Timeline['events'][number]>(`/wifi-surveys/${id}/markers`, {
				marker,
				note: marker === 'note' ? noteDraft : ''
			});
			timeline.events = [...timeline.events, ev];
			if (marker === 'note') noteDraft = '';
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function saveNote(eventId: string, note: string) {
		try {
			await api.put(`/wifi-surveys/${id}/markers/${eventId}`, { note });
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function seek(offsetMs: number) {
		if (!audio) return;
		audio.currentTime = Math.max(0, (offsetMs - 3000) / 1000);
		void audio.play().catch(() => {});
		audio.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
	}

	async function reanalyse() {
		try {
			await api.post(`/wifi-surveys/${id}/analyse`);
			await loadDetails();
			poll();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove() {
		if (!confirm(t('wifi.confirmDelete'))) return;
		try {
			await api.del(`/wifi-surveys/${id}`);
			goto('/wifi-surveys');
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function recText(r: Recommendation): string {
		const v: Record<string, string | number> = {};
		for (const [k, value] of Object.entries(r.values)) {
			v[k] = Array.isArray(value)
				? value.join(', ')
				: value == null
					? '—'
					: (value as string | number);
		}
		return t(`wifi.rec.${r.id}` as MessageKey, v);
	}

	const elapsed = $derived(
		survey?.answered_at ? Math.max(0, tick - new Date(survey.answered_at).getTime()) : 0
	);
	const chartFrom = $derived(
		active && !fullChart ? Math.max(0, (now?.offset_ms ?? elapsed) - 120_000) : 0
	);
	const markers = $derived(timeline.events.filter((e) => e.kind === 'marker'));
	const lightClass = (r: string | null | undefined) =>
		r === 'bad'
			? 'bg-red-600 text-white'
			: r === 'audible'
				? 'bg-amber-400 text-slate-900'
				: r === 'clean'
					? 'bg-emerald-600 text-white'
					: 'bg-slate-300 text-slate-800 dark:bg-slate-700 dark:text-slate-100';
	const tileValue = 'text-2xl font-semibold tabular-nums';
	const events = $derived.by(() => {
		const list: { at: number; text: string; cls: string }[] = [];
		for (const r of roams) {
			list.push({
				at: r.offset_ms,
				text: `${apLabel(r.from_ap_name, r.from_ap_mac)} → ${apLabel(r.to_ap_name, r.to_ap_mac)} · ${r.gap_ms} ms · ${t(`wifi.rating.${r.rating}`)}`,
				cls: ratingBadge(r.rating)
			});
		}
		for (const p of summary?.patterns ?? []) {
			if (p.kind === 'drop_without_roam')
				list.push({
					at: p.start_ms,
					text: t('wifi.pattern.drop_without_roam') + ` · ${p.value} ms`,
					cls: 'badge badge-bad'
				});
		}
		for (const m of markers) {
			list.push({
				at: m.offset_ms,
				text: t(`wifi.marker.${m.marker ?? 'note'}`) + (m.note ? `: ${m.note}` : ''),
				cls: 'badge badge-muted'
			});
		}
		const err = timeline.radio.at(-1)?.error;
		if (active && err)
			list.push({
				at: timeline.radio.at(-1)!.offset_ms,
				text: t('wifi.radioError', { error: err }),
				cls: 'badge badge-warn'
			});
		return list.sort((a, b) => b.at - a.at).slice(0, 30);
	});

	let clockTimer: ReturnType<typeof setInterval> | undefined;
	onMount(async () => {
		try {
			await loadDetails();
		} catch (err) {
			error = errorMessage(err);
		}
		if (survey && ACTIVE.includes(survey.state)) requestWakeLock();
		poll();
		clockTimer = setInterval(() => (tick = Date.now()), 1000);
		document.addEventListener('visibilitychange', visibility);
	});
	onDestroy(() => {
		stopped = true;
		clearTimeout(timer);
		clearInterval(clockTimer);
		releaseWakeLock();
		if (typeof document !== 'undefined')
			document.removeEventListener('visibilitychange', visibility);
	});
</script>

<div class="space-y-4">
	<PageHeader back={{ href: '/wifi-surveys', label: t('wifi.title') }}>
		{#snippet heading()}
			<h1 class="flex flex-wrap items-center gap-2">
				{survey?.device_label ?? t('wifi.title')}
				{#if survey}<span class={stateBadge(survey.state)}>{t(`wifi.state.${survey.state}`)}</span
					>{/if}
			</h1>
		{/snippet}
		{#snippet actions()}
			{#if survey && !active}
				<a class="btn" href={`/api/v1/wifi-surveys/${id}/export?format=csv`} download>CSV</a>
				<a class="btn" href={`/api/v1/wifi-surveys/${id}/export?format=json`} download>JSON</a>
				{#if can('wifi_survey.manage') && survey.answered_at}
					<button class="btn" onclick={reanalyse}>{t('wifi.reanalyse')}</button>
				{/if}
				{#if can('wifi_survey.manage')}
					<button class="btn btn-danger" onclick={remove}>{t('common.delete')}</button>
				{/if}
			{/if}
		{/snippet}
	</PageHeader>
	<ErrorBox {error} />

	{#if survey && active}
		<!-- Live dashboard: all values on one phone screen. -->
		<section class="card sticky top-[var(--header-h)] z-10 flex flex-wrap items-center gap-3 p-3">
			<div
				class="flex h-14 min-w-28 items-center justify-center rounded-xl px-3 text-center text-sm font-semibold {lightClass(
					now?.rating_5s
				)}"
				role="status"
				aria-live="polite"
			>
				{now?.rating_5s ? t(`wifi.rating.${now.rating_5s}`) : t(`wifi.state.${survey.state}`)}
			</div>
			<div class="min-w-0 flex-1">
				<div class="text-2xl font-semibold tabular-nums">{clock(elapsed)}</div>
				<div class="text-xs text-slate-500 dark:text-slate-400">
					{#if offline}
						<span class="text-red-600 dark:text-red-400"
							>{t('wifi.offline', { time: lastUpdate?.toLocaleTimeString() ?? '—' })}</span
						>
					{:else if survey.state === 'armed'}
						{t('wifi.armedHint', { until: formatDateTime(survey.armed_until) })}
					{:else if survey.state === 'calling'}
						{t('wifi.callingHint')}
					{:else}
						{t('wifi.liveHint')}
					{/if}
				</div>
			</div>
			<button class="btn btn-danger" onclick={stop}
				>{survey.state === 'armed' ? t('common.cancel') : t('wifi.stop')}</button
			>
		</section>
		{#if wakeLockFailed}
			<p class="text-xs text-amber-700 dark:text-amber-400">{t('wifi.noWakeLock')}</p>
		{/if}

		{#if survey.state === 'running'}
			<div class="grid grid-cols-2 gap-3 @lg:grid-cols-3 @3xl:grid-cols-4">
				<div class="card p-3">
					<div class="text-xs text-slate-500">{t('wifi.tile.ap')}</div>
					<div class="truncate text-lg font-semibold">
						{now?.radio?.ap_mac ? apLabel(now.radio.ap_name, now.radio.ap_mac) : '—'}
					</div>
					<div class="text-xs text-slate-500">
						{band(now?.radio?.radio)} · {t('wifi.channel')}
						{now?.radio?.channel ?? '—'}
					</div>
				</div>
				<div class="card p-3">
					<div class="text-xs text-slate-500">RSSI</div>
					<div
						class="{tileValue} {rssiLevel(now?.radio?.rssi) === 'bad'
							? 'text-red-600 dark:text-red-400'
							: rssiLevel(now?.radio?.rssi) === 'audible'
								? 'text-amber-600'
								: ''}"
					>
						{now?.radio?.rssi ?? '—'} <span class="text-sm font-normal">dBm</span>
					</div>
					{#if now?.radio?.error}<div class="text-xs text-amber-600">{now.radio.error}</div>{/if}
				</div>
				<div class="card p-3">
					<div class="text-xs text-slate-500">{t('wifi.tile.rate')}</div>
					<div class={tileValue}>
						{mbit(now?.radio?.tx_rate_kbps)}/{mbit(now?.radio?.rx_rate_kbps)}
						<span class="text-sm font-normal">Mbit/s</span>
					</div>
				</div>
				<div class="card p-3">
					<div class="text-xs text-slate-500">{t('wifi.tile.packets')}</div>
					<div class={tileValue}>{now?.received ?? '—'}/{now?.expected ?? '—'}</div>
					<div class="text-xs text-slate-500">{t('wifi.tile.packetsHint')}</div>
				</div>
				<div class="card p-3">
					<div class="text-xs text-slate-500">{t('wifi.tile.uplink')}</div>
					<div
						class="{tileValue} {(now?.loss_10s_pct ?? 0) > 5
							? 'text-red-600 dark:text-red-400'
							: (now?.loss_10s_pct ?? 0) > 0
								? 'text-amber-600'
								: ''}"
					>
						{(now?.loss_10s_pct ?? 0).toFixed(1)} %
					</div>
					<div class="text-xs text-slate-500">
						{t('wifi.tile.gap', { ms: now?.gap_10s_ms ?? 0 })}
					</div>
				</div>
				<div class="card p-3">
					<div class="text-xs text-slate-500">{t('wifi.tile.downlink')}</div>
					<div class={tileValue}>
						{now?.rtcp_fraction_lost != null ? `${now.rtcp_fraction_lost.toFixed(1)} %` : '—'}
					</div>
					<div class="text-xs text-slate-500">
						{t('wifi.tile.jitter')}: {now?.rtcp_jitter_ms != null
							? `${now.rtcp_jitter_ms.toFixed(0)} ms`
							: '—'}
					</div>
				</div>
				<div class="card p-3">
					<div class="text-xs text-slate-500">{t('wifi.tile.mos')}</div>
					<div class={tileValue}>{now?.mos != null ? now.mos.toFixed(2) : '—'}</div>
				</div>
				<div class="card p-3">
					<div class="text-xs text-slate-500">{t('wifi.tile.roams')}</div>
					<div class={tileValue}>{liveSummary?.roams ?? 0}</div>
					{#if roams.at(-1)}
						{@const r = roams.at(-1)!}
						<div class="truncate text-xs">
							{apLabel(r.from_ap_name, r.from_ap_mac)} → {apLabel(r.to_ap_name, r.to_ap_mac)} ·
							{r.gap_ms} ms
							<span class={ratingBadge(r.rating)}>{t(`wifi.rating.${r.rating}`)}</span>
						</div>
					{/if}
				</div>
			</div>

			<section class="card space-y-2 p-3">
				<div class="grid grid-cols-2 gap-2 @xl:flex @xl:flex-wrap">
					<button class="btn" onclick={() => mark('disturbance')}
						>! {t('wifi.marker.disturbance')}</button
					>
					<button class="btn" onclick={() => mark('location')}>⌖ {t('wifi.marker.location')}</button
					>
					<div class="col-span-2 flex min-w-0 flex-1 gap-2">
						<input
							class="input min-w-0"
							maxlength="200"
							placeholder={t('wifi.notePlaceholder')}
							aria-label={t('wifi.marker.note')}
							bind:value={noteDraft}
						/>
						<button class="btn shrink-0" onclick={() => mark('note')}
							>✎ {t('wifi.marker.note')}</button
						>
					</div>
				</div>
				<p class="text-xs text-slate-500 dark:text-slate-400">{t('wifi.dtmfHint')}</p>
			</section>

			<section class="card space-y-2 p-3">
				<div class="flex flex-wrap items-center justify-between gap-2">
					<h2>{t('wifi.timeline')}</h2>
					<div class="flex items-center gap-3 text-sm">
						<label class="flex items-center gap-1 font-normal"
							><input type="checkbox" bind:checked={fullChart} />{t('wifi.fullTimeline')}</label
						>
						<label class="flex items-center gap-1 font-normal"
							><input type="checkbox" bind:checked={soundAlert} />{t('wifi.soundAlert')}</label
						>
					</div>
				</div>
				<WifiSurveyChart {timeline} {roams} from={chartFrom} />
			</section>

			<section class="card p-3">
				<h2 class="mb-2">{t('wifi.events')}</h2>
				{#if events.length}
					<ul class="space-y-1 text-sm">
						{#each events as e, i (i)}
							<li class="flex items-center gap-2">
								<span class="w-14 shrink-0 tabular-nums text-slate-500">{clock(e.at)}</span>
								<span class="{e.cls} truncate">{e.text}</span>
							</li>
						{/each}
					</ul>
				{:else}
					<p class="text-sm text-slate-500">{t('wifi.noEvents')}</p>
				{/if}
			</section>
		{/if}
	{:else if survey}
		<!-- Result. -->
		{#if survey.error}
			<p class="error-box">{survey.error}</p>
		{/if}
		{#if survey.state === 'analysing'}
			<p class="card p-4 text-sm" role="status">{t('wifi.analysing')}</p>
		{/if}
		{#if summary}
			{#if summary.vad_suspected}
				<p class="error-box">{t('wifi.vadWarning')}</p>
			{/if}
			<section class="card space-y-3 p-4">
				<h2>{t('wifi.summary')}</h2>
				<dl class="grid grid-cols-2 gap-3 text-sm @lg:grid-cols-4">
					<div>
						<dt class="text-slate-500">{t('wifi.col.duration')}</dt>
						<dd class="text-lg font-semibold">{clock(summary.duration_ms)}</dd>
					</div>
					<div>
						<dt class="text-slate-500">{t('wifi.col.roams')}</dt>
						<dd class="flex flex-wrap gap-1 text-lg font-semibold">
							{summary.roams}
							<span class="badge badge-ok">{summary.clean}</span>
							<span class="badge badge-warn">{summary.audible}</span>
							<span class="badge badge-bad">{summary.bad}</span>
						</dd>
					</div>
					<div>
						<dt class="text-slate-500">{t('wifi.worstGap')}</dt>
						<dd class="text-lg font-semibold">{summary.worst_gap_ms} ms</dd>
					</div>
					<div>
						<dt class="text-slate-500">{t('wifi.loss')}</dt>
						<dd class="text-lg font-semibold">{summary.loss_pct.toFixed(2)} %</dd>
					</div>
					<div>
						<dt class="text-slate-500">{t('wifi.tile.mos')}</dt>
						<dd class="text-lg font-semibold">{summary.mos?.toFixed(2) ?? '—'}</dd>
					</div>
					<div>
						<dt class="text-slate-500">RSSI min / Ø</dt>
						<dd class="text-lg font-semibold">
							{summary.min_rssi ?? '—'} / {summary.avg_rssi ?? '—'} dBm
						</dd>
					</div>
					<div>
						<dt class="text-slate-500">{t('wifi.markers')}</dt>
						<dd class="text-lg font-semibold">{summary.markers}</dd>
					</div>
					<div>
						<dt class="text-slate-500">{t('wifi.codec')}</dt>
						<dd class="text-lg font-semibold">{survey.codec ?? '—'} · {survey.ptime_ms} ms</dd>
					</div>
				</dl>
				<p class="text-xs text-slate-500 dark:text-slate-400">
					{formatDateTime(survey.answered_at)} – {formatDateTime(survey.ended_at)}
					{#if survey.label}· {survey.label}{/if}
					· {t('wifi.expires', { date: formatDateTime(survey.expires_at) })}
				</p>
			</section>

			{#if summary.recommendations.length}
				<section class="card space-y-2 p-4">
					<h2>{t('wifi.recommendations')}</h2>
					<ul class="space-y-2 text-sm">
						{#each summary.recommendations as r (r.id)}
							<li class="flex items-start gap-2">
								<span
									class="badge shrink-0 {r.severity === 'critical'
										? 'badge-bad'
										: r.severity === 'warning'
											? 'badge-warn'
											: 'badge-muted'}">{t(`wifi.severity.${r.severity}`)}</span
								>
								<span>{recText(r)}</span>
							</li>
						{/each}
					</ul>
					<p class="text-xs text-slate-500 dark:text-slate-400">{t('wifi.recommendationsHint')}</p>
				</section>
			{/if}
		{/if}

		{#if survey.answered_at}
			<section class="card space-y-3 p-4">
				<h2>{t('wifi.timeline')}</h2>
				<WifiSurveyChart {timeline} {roams} onseek={survey.recording_secs ? seek : undefined} />
				{#if survey.recording_secs}
					<audio
						bind:this={audio}
						class="w-full"
						controls
						preload="none"
						src={`/api/v1/wifi-surveys/${id}/audio`}
					></audio>
					<p class="text-xs text-slate-500 dark:text-slate-400">{t('wifi.recordingHint')}</p>
				{/if}
			</section>

			<section class="card">
				<h2 class="p-4 pb-0">{t('wifi.roamTable')}</h2>
				<div class="scroll-x">
					<table class="table">
						<thead>
							<tr>
								<th>{t('wifi.col.time')}</th>
								<th>{t('wifi.col.fromTo')}</th>
								<th class="col-opt">{t('wifi.col.band')}</th>
								<th>RSSI</th>
								<th>{t('wifi.col.gap')}</th>
								<th class="col-opt">{t('wifi.loss')}</th>
								<th class="col-opt-lg">{t('wifi.tile.jitter')}</th>
								<th class="col-opt-lg">{t('wifi.markers')}</th>
								<th>{t('wifi.col.rating')}</th>
								<th><span class="sr-only">{t('wifi.play')}</span></th>
							</tr>
						</thead>
						<tbody>
							{#if !roams.length}
								<EmptyRow
									cols={10}
									text={summary?.has_radio === false ? t('wifi.noRadio') : t('wifi.noRoams')}
								/>
							{/if}
							{#each roams as r (r.offset_ms)}
								<tr>
									<td class="tabular-nums">{clock(r.offset_ms)}</td>
									<td>
										{apLabel(r.from_ap_name, r.from_ap_mac)} → {apLabel(r.to_ap_name, r.to_ap_mac)}
										{#each r.flags.filter((f) => f !== 'no_drop') as f (f)}
											<span class="badge badge-muted ml-1">{t(`wifi.flag.${f}` as MessageKey)}</span
											>
										{/each}
									</td>
									<td class="col-opt">
										{band(r.from_radio)}/{r.from_channel ?? '—'} → {band(
											r.to_radio
										)}/{r.to_channel ?? '—'}
									</td>
									<td class="tabular-nums">{r.rssi_before ?? '—'} → {r.rssi_after ?? '—'}</td>
									<td class="tabular-nums">{r.gap_ms} ms</td>
									<td class="col-opt tabular-nums">{r.loss_pct.toFixed(1)} %</td>
									<td class="col-opt-lg tabular-nums"
										>{r.jitter_ms != null ? `${r.jitter_ms.toFixed(0)} ms` : '—'}</td
									>
									<td class="col-opt-lg">{r.markers}</td>
									<td><span class={ratingBadge(r.rating)}>{t(`wifi.rating.${r.rating}`)}</span></td>
									<td>
										{#if survey.recording_secs}
											<button class="btn btn-sm" onclick={() => seek(r.offset_ms)}
												>▶ {t('wifi.play')}</button
											>
										{/if}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</section>

			{#if summary?.patterns.length}
				<section class="card space-y-2 p-4">
					<h2>{t('wifi.patterns')}</h2>
					<ul class="space-y-1 text-sm">
						{#each summary.patterns as p, i (i)}
							<li>
								<span class="tabular-nums text-slate-500"
									>{clock(p.start_ms)}–{clock(p.end_ms)}</span
								>
								· {t(`wifi.pattern.${p.kind}`)}
								{#if p.ap}· {p.ap}{/if}{#if p.other_ap}
									↔ {p.other_ap}{/if}
								{#if p.value != null}· {p.value}{p.kind === 'drop_without_roam' ||
									p.kind === 'roam_without_drop'
										? ' ms'
										: p.kind === 'sticky_client' || p.kind === 'weak_coverage'
											? ' dBm'
											: ''}{/if}
							</li>
						{/each}
					</ul>
					<p class="text-xs text-slate-500 dark:text-slate-400">{t('wifi.stickyHint')}</p>
				</section>
			{/if}

			{#if summary?.aps.length}
				<section class="card">
					<h2 class="p-4 pb-0">{t('wifi.aps')}</h2>
					<div class="scroll-x">
						<table class="table">
							<thead
								><tr
									><th>AP</th><th>{t('wifi.col.duration')}</th><th>RSSI Ø</th><th>RSSI min</th></tr
								></thead
							>
							<tbody>
								{#each summary.aps as a (a.ap_mac)}
									<tr>
										<td>{apLabel(a.ap_name, a.ap_mac)}</td>
										<td class="tabular-nums">{clock(a.seconds * 1000)}</td>
										<td class="tabular-nums">{a.avg_rssi ?? '—'} dBm</td>
										<td class="tabular-nums">{a.min_rssi ?? '—'} dBm</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				</section>
			{/if}

			<section class="card">
				<h2 class="p-4 pb-0">{t('wifi.markers')}</h2>
				<div class="scroll-x">
					<table class="table">
						<thead
							><tr
								><th>{t('wifi.col.time')}</th><th>{t('wifi.col.marker')}</th><th
									>{t('wifi.col.note')}</th
								><th></th></tr
							></thead
						>
						<tbody>
							{#if !markers.length}<EmptyRow cols={4} text={t('wifi.noMarkers')} />{/if}
							{#each markers as m (m.id)}
								<tr>
									<td class="tabular-nums">{clock(m.offset_ms)}</td>
									<td
										>{t(`wifi.marker.${m.marker ?? 'note'}`)}
										<span class="text-xs text-slate-500"
											>({m.source === 'dtmf' ? 'DTMF' : 'UI'})</span
										></td
									>
									<td>
										<input
											class="input input-sm"
											maxlength="200"
											aria-label={t('wifi.col.note')}
											value={m.note}
											onchange={(e) => saveNote(m.id, (e.currentTarget as HTMLInputElement).value)}
										/>
									</td>
									<td>
										{#if survey.recording_secs}
											<button class="btn btn-sm" onclick={() => seek(m.offset_ms)}
												>▶ {t('wifi.play')}</button
											>
										{/if}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</section>
		{/if}
	{/if}
</div>
