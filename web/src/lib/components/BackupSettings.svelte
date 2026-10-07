<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	type Settings = {
		enabled: boolean;
		hour: number;
		keep: number;
		include_recordings: boolean;
		last_run_at: string | null;
		last_file: string | null;
		last_error: string | null;
	};
	type BackupFile = { name: string; size: number; created_at: string };
	type Overview = { settings: Settings; files: BackupFile[]; running: boolean };

	let data = $state<Overview | null>(null);
	let error = $state('');
	let info = $state('');
	let timer: ReturnType<typeof setTimeout> | undefined;

	async function load() {
		try {
			data = await api.get<Overview>('/backups');
		} catch (err) {
			error = errorMessage(err);
		}
		clearTimeout(timer);
		if (data?.running) timer = setTimeout(load, 2000);
	}

	onMount(load);
	onDestroy(() => clearTimeout(timer));

	function size(bytes: number): string {
		if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(1)} GB`;
		return `${(bytes / 1048576).toFixed(1)} MB`;
	}

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!data) return;
		error = '';
		info = '';
		try {
			const s = data.settings;
			data.settings = await api.put<Settings>('/backups/settings', {
				enabled: s.enabled,
				hour: Number(s.hour),
				keep: Number(s.keep),
				include_recordings: s.include_recordings
			});
			info = t('common.saved');
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function runNow() {
		error = '';
		info = '';
		try {
			await api.post('/backups', {});
			info = t('backup.started');
			setTimeout(load, 500);
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove(file: BackupFile) {
		if (!confirm(t('common.confirmDelete', { name: file.name }))) return;
		error = '';
		try {
			await api.del(`/backups/${encodeURIComponent(file.name)}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

{#if data}
	<form class="card space-y-4" onsubmit={save}>
		<h2>{t('backup.title')}</h2>
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('backup.hint')}</p>
		<ErrorBox {error} />
		{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400">{info}</p>{/if}
		<label class="flex items-center gap-2 font-medium"
			><input type="checkbox" bind:checked={data.settings.enabled} /> {t('backup.daily')}</label
		>
		<div class="grid gap-3 sm:grid-cols-3">
			<div>
				<label for="bk-hour">{t('backup.hour')}</label>
				<input
					id="bk-hour"
					class="input"
					type="number"
					min="0"
					max="23"
					bind:value={data.settings.hour}
				/>
			</div>
			<div>
				<label for="bk-keep">{t('backup.keep')}</label>
				<input
					id="bk-keep"
					class="input"
					type="number"
					min="1"
					max="365"
					bind:value={data.settings.keep}
				/>
			</div>
			<label class="flex items-center gap-2 self-end pb-2"
				><input type="checkbox" bind:checked={data.settings.include_recordings} />
				{t('backup.recordings')}</label
			>
		</div>
		<p class="hint">
			{t('backup.lastRun')}: {formatDateTime(data.settings.last_run_at)}
			{#if data.settings.last_error}
				<span class="text-red-700 dark:text-red-400">· {data.settings.last_error}</span>
			{/if}
		</p>
		<div class="flex flex-wrap gap-2">
			<button class="btn btn-primary" type="submit">{t('common.save')}</button>
			<button class="btn" type="button" onclick={runNow} disabled={data.running}>
				{data.running ? t('backup.running') : t('backup.now')}
			</button>
		</div>
		{#if data.files.length}
			<div class="overflow-x-auto">
				<table class="table">
					<thead>
						<tr>
							<th>{t('backup.file')}</th>
							<th>{t('backup.size')}</th>
							<th><span class="sr-only">{t('backup.actions')}</span></th>
						</tr>
					</thead>
					<tbody>
						{#each data.files as f (f.name)}
							<tr>
								<td>
									<span class="font-mono text-sm">{f.name}</span>
									<span class="block text-xs text-slate-500">{formatDateTime(f.created_at)}</span>
								</td>
								<td class="text-sm whitespace-nowrap">{size(f.size)}</td>
								<td class="text-right">
									<div class="flex flex-col items-end gap-1 sm:flex-row sm:justify-end">
										<a
											class="btn btn-sm"
											href={`/api/v1/backups/${encodeURIComponent(f.name)}`}
											download>{t('backup.download')}</a
										>
										<button class="btn btn-sm btn-danger" type="button" onclick={() => remove(f)}
											>{t('common.delete')}</button
										>
									</div>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else}
			<p class="hint">{t('backup.none')}</p>
		{/if}
		<p class="hint">{t('backup.restoreHint')}</p>
	</form>
{/if}
