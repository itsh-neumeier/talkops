<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, i18n, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import {
		hasDay,
		runTimes,
		toggleDay,
		type Retention,
		type Schedule,
		type StorageOverview,
		type StorageTarget
	} from '#lib/backupTargets.ts';

	type ExportFile = {
		id: number;
		file: string;
		first_id: number;
		last_id: number;
		entries: number;
		bytes: number;
		sha256: string;
		created_at: string;
	};
	type View = {
		retention_days: number;
		export_enabled: boolean;
		storage_target_id: string | null;
		folder: string;
		format: 'csv' | 'json';
		schedule: Schedule;
		retention: Retention;
		last_exported_id: number;
		last_run_at: string | null;
		last_success_at: string | null;
		last_error: string | null;
		running: boolean;
		next_run_at: string | null;
		files: ExportFile[];
		transfers_available: boolean;
	};

	let view = $state<View | null>(null);
	let draft = $state<{
		retention_days: number;
		export_enabled: boolean;
		storage_target_id: string;
		folder: string;
		format: 'csv' | 'json';
		schedule: Schedule;
		retention: Retention;
	} | null>(null);
	let targets = $state<StorageTarget[]>([]);
	let error = $state('');
	let notice = $state('');
	let busy = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	function apply(v: View) {
		view = v;
		draft = {
			retention_days: v.retention_days,
			export_enabled: v.export_enabled,
			storage_target_id: v.storage_target_id ?? '',
			folder: v.folder,
			format: v.format,
			schedule: { ...v.schedule },
			retention: { ...v.retention }
		};
	}

	async function load() {
		try {
			const [v, storage] = await Promise.all([
				api.get<View>('/audit/settings'),
				api.get<StorageOverview>('/storage-targets').catch(() => null)
			]);
			targets = storage?.targets ?? [];
			apply(v);
			error = '';
		} catch (err) {
			error = errorMessage(err);
		}
	}

	/** Reloads the status while a run is active. */
	async function poll() {
		clearTimeout(timer);
		try {
			const v = await api.get<View>('/audit/settings');
			view = v;
			if (v.running) timer = setTimeout(poll, 2000);
		} catch (err) {
			error = errorMessage(err);
		}
	}

	onMount(load);
	onDestroy(() => clearTimeout(timer));

	const weekdays = $derived(
		Array.from({ length: 7 }, (_, i) =>
			new Intl.DateTimeFormat(i18n.locale === 'de' ? 'de-DE' : 'en-GB', {
				weekday: 'short'
			}).format(new Date(2024, 0, 1 + i))
		)
	);

	function setTime(value: string) {
		if (!draft) return;
		const [h, m] = value.split(':').map(Number);
		if (Number.isFinite(h) && Number.isFinite(m)) {
			draft.schedule.hour = h;
			draft.schedule.minute = m;
		}
	}

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!draft) return;
		busy = true;
		notice = '';
		try {
			const body = $state.snapshot(draft);
			apply(
				await api.put<View>('/audit/settings', {
					...body,
					storage_target_id: body.storage_target_id || null
				})
			);
			notice = t('audit.saved');
			error = '';
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	async function runNow() {
		busy = true;
		notice = '';
		try {
			await api.post('/audit/export/run');
			notice = t('audit.started');
			error = '';
			setTimeout(poll, 1000);
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	const sizeText = (bytes: number) =>
		bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KiB`;
</script>

<ErrorBox {error} />
{#if notice}<p class="text-sm text-teal-700 dark:text-teal-300" role="status">{notice}</p>{/if}
{#if draft && view}
	<form class="space-y-4" onsubmit={save}>
		<section class="card space-y-3 p-4">
			<h2 class="text-base font-semibold">{t('audit.retention')}</h2>
			<div class="max-w-xs">
				<label for="audit-days">{t('audit.retentionDays')}</label>
				<input
					id="audit-days"
					class="input"
					type="number"
					min="0"
					max="36500"
					bind:value={draft.retention_days}
					required
				/>
			</div>
			<p class="hint">{t('audit.retentionHint')}</p>
		</section>

		<section class="card space-y-3 p-4">
			<h2 class="text-base font-semibold">{t('audit.export')}</h2>
			{#if !view.transfers_available}<p class="hint">{t('audit.noTransfers')}</p>{/if}
			<label class="flex items-center gap-2"
				><input type="checkbox" bind:checked={draft.export_enabled} />
				{t('audit.exportEnabled')}</label
			>
			<p class="hint">{t('audit.exportHint')}</p>
			{#if targets.length === 0}<p class="hint">{t('audit.noStorage')}</p>{/if}
			<div class="grid grid-cols-1 gap-3 sm:grid-cols-3">
				<div>
					<label for="audit-target">{t('storage.target')}</label>
					<select
						id="audit-target"
						class="input"
						bind:value={draft.storage_target_id}
						required={draft.export_enabled}
					>
						<option value="">{t('audit.noTarget')}</option>
						{#each targets as x (x.id)}<option value={x.id}>{x.name}</option>{/each}
					</select>
				</div>
				<div>
					<label for="audit-folder">{t('audit.folder')}</label>
					<input id="audit-folder" class="input" bind:value={draft.folder} maxlength="1024" />
				</div>
				<div>
					<label for="audit-format">{t('audit.format')}</label>
					<select id="audit-format" class="input" bind:value={draft.format}>
						<option value="json">{t('audit.formatJson')}</option>
						<option value="csv">{t('audit.formatCsv')}</option>
					</select>
				</div>
			</div>
			<fieldset class="space-y-2 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
				<legend class="px-1 text-sm font-medium">{t('targets.schedule')}</legend>
				<div class="flex flex-wrap gap-1" role="group" aria-label={t('targets.weekdays')}>
					{#each weekdays as day, i (i)}
						<button
							type="button"
							class="btn btn-sm {hasDay(draft.schedule.weekdays, i) ? 'btn-primary' : ''}"
							aria-pressed={hasDay(draft.schedule.weekdays, i)}
							onclick={() =>
								draft && (draft.schedule.weekdays = toggleDay(draft.schedule.weekdays, i))}
							>{day}</button
						>
					{/each}
				</div>
				<div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
					<div>
						<label for="audit-time">{t('targets.firstRun')}</label>
						<input
							id="audit-time"
							class="input"
							type="time"
							value={`${String(draft.schedule.hour).padStart(2, '0')}:${String(draft.schedule.minute).padStart(2, '0')}`}
							onchange={(e) => setTime(e.currentTarget.value)}
						/>
					</div>
					<div>
						<label for="audit-every">{t('targets.repeat')}</label>
						<select id="audit-every" class="input" bind:value={draft.schedule.every_hours}>
							<option value={0}>{t('targets.onceDaily')}</option>
							{#each [1, 2, 3, 4, 6, 8, 12] as h (h)}<option value={h}
									>{t('targets.everyHours', { n: h })}</option
								>{/each}
						</select>
					</div>
				</div>
				<p class="hint">
					{draft.schedule.weekdays === 0
						? t('targets.manualOnly')
						: `${t('targets.runsAt')}: ${runTimes(draft.schedule).join(', ')}`}
				</p>
			</fieldset>
			<fieldset class="space-y-2 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
				<legend class="px-1 text-sm font-medium">{t('audit.fileRetention')}</legend>
				<p class="hint">{t('targets.retentionHint')}</p>
				<div class="grid grid-cols-2 gap-3 sm:grid-cols-5">
					{#each [['keep_last', 'targets.keepLast', 1, 1000], ['keep_daily', 'targets.keepDaily', 0, 3660], ['keep_weekly', 'targets.keepWeekly', 0, 520], ['keep_monthly', 'targets.keepMonthly', 0, 1200], ['keep_yearly', 'targets.keepYearly', 0, 100]] as const as [field, label, min, max] (field)}
						<div>
							<label for={`audit-${field}`}>{t(label)}</label>
							<input
								id={`audit-${field}`}
								class="input"
								type="number"
								{min}
								{max}
								bind:value={draft.retention[field]}
								required
							/>
						</div>
					{/each}
				</div>
			</fieldset>
			<dl class="grid grid-cols-1 gap-x-4 gap-y-1 text-sm sm:grid-cols-2">
				<div>
					<dt class="inline text-slate-500">{t('audit.lastRun')}:</dt>
					<dd class="inline">
						{view.last_run_at ? formatDateTime(view.last_run_at) : '—'}
						{#if view.last_error}<span class="block text-red-600 dark:text-red-400"
								>{view.last_error}</span
							>{/if}
					</dd>
				</div>
				<div>
					<dt class="inline text-slate-500">{t('audit.nextRun')}:</dt>
					<dd class="inline">{view.next_run_at ? formatDateTime(view.next_run_at) : '—'}</dd>
				</div>
				<div>
					<dt class="inline text-slate-500">{t('audit.lastExported')}:</dt>
					<dd class="inline">{view.last_exported_id || '—'}</dd>
				</div>
			</dl>
		</section>

		<div class="flex flex-wrap justify-end gap-2">
			<button
				type="button"
				class="btn"
				disabled={busy || view.running || !view.storage_target_id || !view.transfers_available}
				onclick={runNow}>{t('audit.runNow')}</button
			>
			<button class="btn btn-primary" disabled={busy}>{t('common.save')}</button>
		</div>
	</form>

	{#if view.files.length}
		<section class="card scroll-x">
			<h2 class="p-4 pb-0 text-base font-semibold">{t('audit.files')}</h2>
			<table class="table">
				<thead>
					<tr
						><th>{t('audit.time')}</th><th>{t('audit.file')}</th><th class="col-opt"
							>{t('audit.entries')}</th
						><th class="col-opt-lg">{t('audit.checksum')}</th></tr
					>
				</thead>
				<tbody>
					{#each view.files as f (f.id)}
						<tr>
							<td class="whitespace-nowrap">{formatDateTime(f.created_at)}</td>
							<td class="font-mono text-xs break-all"
								>{f.file}<span class="block text-slate-500">{sizeText(f.bytes)}</span></td
							>
							<td class="col-opt"
								>{f.entries}
								<span class="text-xs text-slate-500">#{f.first_id}–{f.last_id}</span></td
							>
							<td class="col-opt-lg font-mono text-xs break-all">{f.sha256}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</section>
	{/if}
{/if}
