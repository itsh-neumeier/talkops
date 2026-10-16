<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import type { Capture, DiagnosticsView, LogLine } from '#lib/diagnostics.ts';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { copy, errorMessage } from '#lib/util.ts';

	let capture = $state<Capture | null>(null);
	let error = $state('');
	let level = $state('debug');
	let sipTrace = $state(true);
	let minutes = $state(10);
	let lines = $state<LogLine[]>([]);
	let filter = $state('');
	let follow = $state(true);
	let logBox = $state<HTMLPreElement>();
	let timer: ReturnType<typeof setInterval> | undefined;

	const MAX_SHOWN = 5000;

	async function load() {
		try {
			capture = (await api.get<DiagnosticsView>('/diagnostics')).capture;
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function poll() {
		try {
			const after = lines.length ? lines[lines.length - 1].seq : 0;
			const page = await api.get<{ lines: LogLine[]; capture: Capture }>(
				`/diagnostics/log?after=${after}`
			);
			capture = page.capture;
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
			capture = await api.post<Capture>('/diagnostics/capture', {
				level,
				sip_trace: sipTrace,
				minutes: Number(minutes)
			});
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

	const shown = $derived(
		filter.trim()
			? lines.filter((l) => l.text.toLowerCase().includes(filter.trim().toLowerCase()))
			: lines
	);
</script>

<ErrorBox {error} />

<section class="card space-y-3">
	<h2>{t('diag.capture')}</h2>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('diag.captureHint')}</p>
	<div class="flex flex-wrap items-end gap-3">
		<div>
			<label for="d-level">{t('diag.level')}</label>
			<select id="d-level" class="input" bind:value={level} disabled={capture?.running}>
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
				disabled={capture?.running}
			/>
		</div>
		<label class="flex items-center gap-2 pb-2"
			><input type="checkbox" bind:checked={sipTrace} disabled={capture?.running} />
			{t('diag.sipTrace')}</label
		>
		{#if capture?.running}
			<button class="btn btn-danger" onclick={stop}>{t('diag.stop')}</button>
		{:else}
			<button class="btn btn-primary" onclick={start}>{t('diag.start')}</button>
		{/if}
	</div>
	{#if capture?.running && capture.until}
		<p class="text-sm text-emerald-700 dark:text-emerald-400" data-testid="capture-running">
			● {t('diag.running', {
				level: capture.level ?? '',
				until: formatDateTime(capture.until)
			})}{capture.sip_trace ? ` · ${t('diag.sipTrace')}` : ''}
		</p>
	{/if}

	<div class="flex flex-wrap items-center gap-2">
		<input
			class="input mt-0 max-w-xs min-w-0 flex-1"
			placeholder={t('diag.filter')}
			aria-label={t('diag.filter')}
			bind:value={filter}
		/>
		<label class="flex items-center gap-2 text-sm"
			><input type="checkbox" bind:checked={follow} /> {t('diag.follow')}</label
		>
		<div class="ml-auto flex flex-wrap gap-2">
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
