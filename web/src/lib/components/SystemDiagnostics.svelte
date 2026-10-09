<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { copy, errorMessage } from '#lib/util.ts';

	type Capture = {
		running: boolean;
		level: string | null;
		sip_trace: boolean;
		started_at: string | null;
		until: string | null;
		lines: number;
		last_seq: number;
	};
	type Channel = {
		uuid: string;
		created: string;
		name: string;
		state: string;
		callstate: string;
		cid_name: string;
		cid_num: string;
		dest: string;
		application: string;
		application_data: string;
		read_codec: string;
		write_codec: string;
		secure: string;
	};
	type View = {
		version: string;
		server_started_at: string;
		freeswitch: string | null;
		capture: Capture;
		channels: Channel[];
	};
	type Line = { seq: number; text: string };

	let view = $state<View | null>(null);
	let error = $state('');
	let level = $state('debug');
	let sipTrace = $state(true);
	let minutes = $state(10);
	let lines = $state<Line[]>([]);
	let filter = $state('');
	let follow = $state(true);
	let restarting = $state('');
	let logBox = $state<HTMLPreElement>();
	let timer: ReturnType<typeof setInterval> | undefined;
	let now = $state(Date.now());

	const MAX_SHOWN = 5000;

	/** Public test targets (sip5060.net): sound and NAT without a provider. */
	const testCalls = [
		{ uri: 'test.echo@sip5060.net', label: 'diag.test.echo' },
		{ uri: 'test.dtmf@sip5060.net', label: 'diag.test.dtmf' },
		{ uri: 'test.time@sip5060.net', label: 'diag.test.time' },
		{ uri: 'test.ring@sip5060.net', label: 'diag.test.ring' }
	] as const;

	async function load() {
		try {
			view = await api.get<View>('/diagnostics');
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function poll() {
		now = Date.now();
		try {
			const after = lines.length ? lines[lines.length - 1].seq : 0;
			const page = await api.get<{ lines: Line[]; capture: Capture }>(
				`/diagnostics/log?after=${after}`
			);
			if (view) view.capture = page.capture;
			if (page.lines.length) {
				lines = [...lines, ...page.lines].slice(-MAX_SHOWN);
				if (follow) queueMicrotask(() => logBox?.scrollTo({ top: logBox.scrollHeight }));
			}
		} catch {
			// Server restarting or offline; the next tick retries.
		}
	}

	onMount(async () => {
		await load();
		await poll();
		timer = setInterval(poll, 2000);
	});
	onDestroy(() => clearInterval(timer));

	async function start() {
		error = '';
		try {
			const capture = await api.post<Capture>('/diagnostics/capture', {
				level,
				sip_trace: sipTrace,
				minutes: Number(minutes)
			});
			if (view) view.capture = capture;
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function stop() {
		try {
			await api.del('/diagnostics/capture');
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	let copied = $state<'' | 'ok' | 'failed'>('');

	/** Copies the lines shown (respecting the filter) as plain text. */
	async function copyLog() {
		const text = shown.map((l) => (l.text.endsWith('\n') ? l.text : l.text + '\n')).join('');
		copied = (await copy(text)) ? 'ok' : 'failed';
		setTimeout(() => (copied = ''), 2500);
	}

	async function clearLog() {
		try {
			await api.del('/diagnostics/log');
			lines = [];
		} catch (err) {
			error = errorMessage(err);
		}
	}

	let sessionsInfo = $state('');
	let sessionsBusy = $state(false);

	async function resetSessions(action: 'hangup' | 'reregister' | 'all') {
		const question = {
			hangup: t('diag.hangupConfirm'),
			reregister: t('diag.reregisterConfirm'),
			all: t('diag.sessionsAllConfirm')
		}[action];
		if (!confirm(question)) return;
		error = '';
		sessionsInfo = '';
		sessionsBusy = true;
		try {
			const r = await api.post<{ hung_up: number; reregistered: boolean }>(
				'/diagnostics/sessions',
				{ action }
			);
			sessionsInfo = [
				action !== 'reregister' ? t('diag.hungUp', { n: r.hung_up }) : '',
				r.reregistered ? t('diag.reregistered') : ''
			]
				.filter(Boolean)
				.join(' ');
			await load();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			sessionsBusy = false;
		}
	}

	async function restart(target: 'freeswitch' | 'all') {
		const question = target === 'all' ? t('diag.restartAllConfirm') : t('diag.restartFsConfirm');
		if (!confirm(question)) return;
		error = '';
		try {
			await api.post('/diagnostics/restart', { target });
			restarting = target;
			// Wait until the server answers again, then reload the data.
			const started = Date.now();
			const wait = async () => {
				await new Promise((r) => setTimeout(r, 3000));
				try {
					const res = await fetch('/api/v1/status', { cache: 'no-store' });
					const fsUp =
						target === 'freeswitch' ? (await api.get<View>('/diagnostics')).freeswitch : 'x';
					if (res.ok && fsUp) {
						restarting = '';
						await load();
						return;
					}
				} catch {
					// not back yet
				}
				if (Date.now() - started < 180_000) await wait();
				else restarting = '';
			};
			await wait();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function since(iso: string): string {
		const secs = Math.max(0, Math.floor((now - Date.parse(iso)) / 1000));
		const d = Math.floor(secs / 86400);
		const h = Math.floor((secs % 86400) / 3600);
		const m = Math.floor((secs % 3600) / 60);
		return d ? `${d}d ${h}h ${m}m` : h ? `${h}h ${m}m` : `${m}m`;
	}

	const shown = $derived(
		filter.trim()
			? lines.filter((l) => l.text.toLowerCase().includes(filter.trim().toLowerCase()))
			: lines
	);
	// "UP 0 years, 3 days, 4 hours, 12 minutes, …" → "3d 4h 12m".
	const fsUptime = $derived.by(() => {
		const first = view?.freeswitch?.split('\n')[0] ?? '';
		const n = (unit: string) => Number(first.match(new RegExp(`(\\d+) ${unit}`))?.[1] ?? 0);
		const days = n('years') * 365 + n('days');
		const h = n('hours');
		const m = n('minutes');
		return days ? `${days}d ${h}h ${m}m` : h ? `${h}h ${m}m` : `${m}m`;
	});
</script>

<ErrorBox {error} />

<section class="card space-y-3">
	<h2>{t('diag.overview')}</h2>
	{#if view}
		<dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-sm">
			<dt class="text-slate-500 dark:text-slate-400">{t('diag.version')}</dt>
			<dd class="font-mono">{view.version}</dd>
			<dt class="text-slate-500 dark:text-slate-400">{t('diag.serverUptime')}</dt>
			<dd>{since(view.server_started_at)}</dd>
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
		</dl>
	{/if}
</section>

<section class="card space-y-3">
	<h2>{t('diag.capture')}</h2>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('diag.captureHint')}</p>
	<div class="flex flex-wrap items-end gap-3">
		<div>
			<label for="d-level">{t('diag.level')}</label>
			<select id="d-level" class="input" bind:value={level} disabled={view?.capture.running}>
				<option value="debug">Debug</option>
				<option value="info">Info</option>
				<option value="notice">Notice</option>
				<option value="warning">Warning</option>
			</select>
		</div>
		<div>
			<label for="d-min">{t('diag.minutes')}</label>
			<input
				id="d-min"
				class="input w-24"
				type="number"
				min="1"
				max="60"
				bind:value={minutes}
				disabled={view?.capture.running}
			/>
		</div>
		<label class="flex items-center gap-2 pb-2"
			><input type="checkbox" bind:checked={sipTrace} disabled={view?.capture.running} />
			{t('diag.sipTrace')}</label
		>
		{#if view?.capture.running}
			<button class="btn btn-danger" onclick={stop}>{t('diag.stop')}</button>
		{:else}
			<button class="btn btn-primary" onclick={start}>{t('diag.start')}</button>
		{/if}
	</div>
	{#if view?.capture.running && view.capture.until}
		<p class="text-sm text-emerald-700 dark:text-emerald-400" data-testid="capture-running">
			● {t('diag.running', {
				level: view.capture.level ?? '',
				until: formatDateTime(view.capture.until)
			})}{view.capture.sip_trace ? ` · ${t('diag.sipTrace')}` : ''}
		</p>
	{/if}

	<div class="flex flex-wrap items-center gap-2">
		<input
			class="input mt-0 max-w-xs flex-1"
			placeholder={t('diag.filter')}
			aria-label={t('diag.filter')}
			bind:value={filter}
		/>
		<label class="flex items-center gap-2 text-sm"
			><input type="checkbox" bind:checked={follow} /> {t('diag.follow')}</label
		>
		<div class="ml-auto flex gap-2">
			<button class="btn" disabled={!shown.length} onclick={copyLog} data-testid="copy-log"
				>{copied === 'ok'
					? `✓ ${t('diag.copied')}`
					: copied === 'failed'
						? t('diag.copyFailed')
						: `⧉ ${t('diag.copy')}`}</button
			>
			<a class="btn" href="/api/v1/diagnostics/log.txt" download>⬇ {t('diag.download')}</a>
			<button class="btn" onclick={clearLog}>{t('diag.clear')}</button>
		</div>
	</div>
	<pre
		bind:this={logBox}
		class="h-96 overflow-auto rounded-lg bg-slate-950 p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap text-slate-200"
		data-testid="log">{#if shown.length}{#each shown as l (l.seq)}{l.text.endsWith('\n')
					? l.text
					: l.text + '\n'}{/each}{:else}<span class="text-slate-500">{t('diag.empty')}</span
			>{/if}</pre>
	<p class="hint">{t('diag.privacy')}</p>
</section>

<section class="card space-y-3">
	<div class="flex items-center justify-between">
		<h2>{t('diag.channels')}</h2>
		<button class="btn" onclick={load}>↻ {t('diag.refresh')}</button>
	</div>
	{#if view?.channels.length}
		<div class="overflow-x-auto">
			<table class="w-full text-left text-sm">
				<thead class="text-slate-500 dark:text-slate-400">
					<tr>
						<th class="py-1 pr-3">{t('diag.chName')}</th>
						<th class="py-1 pr-3">{t('diag.chCaller')}</th>
						<th class="py-1 pr-3">{t('diag.chDest')}</th>
						<th class="py-1 pr-3">{t('diag.chState')}</th>
						<th class="py-1 pr-3">Codec</th>
						<th class="py-1 pr-3">{t('diag.chApp')}</th>
					</tr>
				</thead>
				<tbody>
					{#each view.channels as c (c.uuid)}
						<tr class="border-t border-slate-200 align-top dark:border-slate-700">
							<td class="py-1 pr-3 font-mono text-xs break-all">{c.name}</td>
							<td class="py-1 pr-3">{c.cid_name} <span class="font-mono">{c.cid_num}</span></td>
							<td class="py-1 pr-3 font-mono">{c.dest}</td>
							<td class="py-1 pr-3">{c.callstate || c.state}</td>
							<td class="py-1 pr-3 font-mono text-xs"
								>{c.read_codec}{c.secure ? ` · ${c.secure}` : ''}</td
							>
							<td class="py-1 pr-3 font-mono text-xs break-all"
								>{c.application} {c.application_data}</td
							>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{:else}
		<p class="text-sm text-slate-500 dark:text-slate-400">{t('diag.noChannels')}</p>
	{/if}
</section>

<section class="card space-y-3">
	<h2>{t('diag.testCalls')}</h2>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('diag.testCallsHint')}</p>
	<div class="grid gap-2 sm:grid-cols-2">
		{#each testCalls as tc (tc.uri)}
			<a class="btn justify-start text-left" href="/phone?dial={encodeURIComponent(tc.uri)}"
				><span class="font-medium">{t(tc.label)}</span>
				<span class="ml-2 font-mono text-xs text-slate-500 dark:text-slate-400">{tc.uri}</span></a
			>
		{/each}
	</div>
	<p class="hint">{t('diag.testCallsSource')}</p>
</section>

<section class="card space-y-3">
	<h2>{t('diag.sessions')}</h2>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('diag.sessionsHint')}</p>
	{#if sessionsInfo}<p
			class="text-sm text-emerald-700 dark:text-emerald-400"
			data-testid="sessions-info"
		>
			{sessionsInfo}
		</p>{/if}
	<div class="flex flex-wrap gap-2">
		<button class="btn" disabled={sessionsBusy} onclick={() => resetSessions('hangup')}
			>{t('diag.hangupAll')}</button
		>
		<button class="btn" disabled={sessionsBusy} onclick={() => resetSessions('reregister')}
			>{t('diag.reregister')}</button
		>
		<button class="btn btn-danger" disabled={sessionsBusy} onclick={() => resetSessions('all')}
			>{t('diag.sessionsAll')}</button
		>
	</div>
	<p class="hint">{t('diag.sessionsTimer')}</p>
</section>

<section class="card space-y-3">
	<h2>{t('diag.restart')}</h2>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('diag.restartHint')}</p>
	{#if restarting}
		<p class="text-sm text-amber-700 dark:text-amber-400" data-testid="restarting">
			⟳ {t('diag.restarting')}
		</p>
	{/if}
	<div class="flex flex-wrap gap-2">
		<button class="btn" disabled={!!restarting} onclick={() => restart('freeswitch')}
			>{t('diag.restartFs')}</button
		>
		<button class="btn btn-danger" disabled={!!restarting} onclick={() => restart('all')}
			>{t('diag.restartAll')}</button
		>
	</div>
</section>
