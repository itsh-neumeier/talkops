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
	type Deployment = {
		complete: boolean;
		missing: string[];
		source: string;
		captured_at: string;
		project_name: string;
		profiles: string;
		files: string[];
	};
	type Overview = {
		settings: Settings;
		files: BackupFile[];
		running: boolean;
		configuration: Deployment | null;
		configuration_error: string | null;
		encryption_configured: boolean;
	};
	let sourceFiles = $state<File[]>([]);
	let project = $state('talkops');
	let profiles = $state('');
	async function importConfig() {
		error = '';
		info = '';
		try {
			const files: Record<string, string> = {};
			for (const file of sourceFiles) {
				if (file.size > 1048576) throw new Error(t('backup.importHint'));
				files[file.name] = await file.text();
			}
			await api.put('/backups/deployment', { files, project_name: project, profiles });
			sourceFiles = [];
			await load();
			info = t('common.saved');
		} catch (err) {
			error = errorMessage(err);
		}
	}

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
		<div class="space-y-2 rounded border border-slate-300 p-3 dark:border-slate-700">
			<h3>{t('backup.configTitle')}</h3>
			{#if data.configuration_error}<p class="hint">{data.configuration_error}</p>{/if}
			{#if data.configuration}
				<p class="hint">
					{data.configuration.complete ? t('backup.configComplete') : t('backup.configMissing')}
					{data.configuration.missing.join(', ')}
				</p>
				{#if data.configuration.source === 'imported'}<p class="hint">
						{t('backup.configImported')}
						{formatDateTime(data.configuration.captured_at)}
					</p>{/if}
			{/if}
			{#if !data.encryption_configured}<p class="hint">{t('backup.encryptionMissing')}</p>{/if}
			{#if data.configuration?.source !== 'mounted'}
				<h3>{t('backup.importTitle')}</h3>
				<p class="hint">{t('backup.importHint')}</p>
				<label for="bk-config">{t('backup.importFiles')}</label>
				<input
					id="bk-config"
					class="input"
					type="file"
					multiple
					onchange={(e) => (sourceFiles = Array.from(e.currentTarget.files ?? []))}
				/>
				<label for="bk-project">{t('backup.project')}</label><input
					id="bk-project"
					class="input"
					bind:value={project}
				/>
				<label for="bk-profiles">{t('backup.profiles')}</label><input
					id="bk-profiles"
					class="input"
					bind:value={profiles}
				/>
				<button class="btn" type="button" onclick={importConfig} disabled={!sourceFiles.length}
					>{t('backup.importButton')}</button
				>
			{/if}
		</div>
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
