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
		excluded?: string[];
	};
	type Portainer = {
		url: string;
		stack_id: number;
		cert_sha256: string;
		stack_name: string | null;
		last_sync_at: string | null;
		last_error: string | null;
	};
	type Overview = {
		settings: Settings;
		files: BackupFile[];
		running: boolean;
		configuration: Deployment | null;
		configuration_error: string | null;
		encryption_configured: boolean;
		mounted: boolean;
		auto_discovery: boolean;
		portainer: Portainer | null;
	};
	let pUrl = $state('https://127.0.0.1:9443');
	let pToken = $state('');
	let pCert = $state('');
	let pProbe = $state<{ cert_sha256: string | null; trusted: boolean } | null>(null);
	let pPin = $state(false);
	let pStacks = $state<{ id: number; name: string }[]>([]);
	let pStack = $state(0);
	let pBusy = $state(false);
	let pEditing = $state(false);

	function editPortainer() {
		const p = data?.portainer;
		pUrl = p?.url ?? 'https://127.0.0.1:9443';
		pCert = p?.cert_sha256 ?? '';
		pPin = !!pCert;
		pStack = p?.stack_id ?? 0;
		pToken = '';
		pProbe = null;
		pStacks = [];
		pEditing = true;
	}

	async function portainerCall(fn: () => Promise<void>) {
		pBusy = true;
		error = '';
		info = '';
		try {
			await fn();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			pBusy = false;
		}
	}

	const probePortainer = () =>
		portainerCall(async () => {
			pProbe = await api.post('/backups/portainer/certificate', { url: pUrl });
			pPin = false;
			pCert = '';
		});
	const certForRequest = () => (pPin && pProbe?.cert_sha256 ? pProbe.cert_sha256 : pCert);
	const loadStacks = () =>
		portainerCall(async () => {
			pStacks = await api.post('/backups/portainer/stacks', {
				url: pUrl,
				cert_sha256: certForRequest(),
				token: pToken || null
			});
			if (!pStacks.some((x) => x.id === pStack)) pStack = pStacks[0]?.id ?? 0;
		});
	const savePortainer = () =>
		portainerCall(async () => {
			await api.put('/backups/portainer', {
				url: pUrl,
				stack_id: Number(pStack),
				cert_sha256: certForRequest(),
				token: pToken || null
			});
			pEditing = false;
			pToken = '';
			await load();
			info = t('backup.portainer.saved');
		});
	const syncPortainer = () =>
		portainerCall(async () => {
			await api.post('/backups/portainer/sync', {});
			await load();
			info = t('backup.portainer.saved');
		});
	const removePortainer = () =>
		portainerCall(async () => {
			if (!confirm(t('backup.portainer.confirmRemove'))) return;
			await api.del('/backups/portainer');
			await load();
		});
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
		<div class="space-y-2 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
			<h3>{t('backup.configTitle')}</h3>
			{#if data.configuration_error}<p class="hint">{data.configuration_error}</p>{/if}
			{#if data.configuration}
				<p class="hint">
					{t('backup.configSource')}:
					{t(
						data.configuration.source === 'mounted'
							? 'backup.source.mounted'
							: data.configuration.source === 'portainer'
								? 'backup.source.portainer'
								: data.configuration.source === 'imported'
									? 'backup.source.imported'
									: 'backup.source.missing'
					)}
					{#if data.configuration.files.length}
						· <span class="font-mono">{data.configuration.files.join(', ')}</span>{/if}
				</p>
				<p class="hint">
					{data.configuration.complete ? t('backup.configComplete') : t('backup.configMissing')}
					{data.configuration.missing.join(', ')}
				</p>
				{#if data.configuration.excluded?.length}<p class="hint">
						{t('backup.configExcluded')}
						<span class="font-mono">{data.configuration.excluded.join(', ')}</span>
					</p>{/if}
				{#if data.configuration.source === 'imported'}<p class="hint">
						{t('backup.configImported')}
						{formatDateTime(data.configuration.captured_at)}
					</p>{/if}
			{/if}
			{#if !data.encryption_configured}<p class="hint">{t('backup.encryptionMissing')}</p>{/if}
			{#if !data.mounted}
				<h3>{t('backup.portainer.title')}</h3>
				<p class="hint">{t('backup.portainer.hint')}</p>
				{#if data.portainer && !pEditing}
					<p class="text-sm">
						<span class="font-mono">{data.portainer.url}</span> · {t('backup.portainer.stack')}:
						{data.portainer.stack_name ?? data.portainer.stack_id}
						{#if data.portainer.cert_sha256}· {t('backup.portainer.pinned')}{/if}
					</p>
					<p class="hint">
						{t('backup.portainer.lastSync')}: {formatDateTime(data.portainer.last_sync_at)}
						{#if data.portainer.last_error}<span class="text-red-700 dark:text-red-400"
								>· {data.portainer.last_error}</span
							>{/if}
					</p>
					<div class="flex flex-wrap gap-2">
						<button class="btn" type="button" disabled={pBusy} onclick={syncPortainer}
							>{t('backup.portainer.syncNow')}</button
						>
						<button class="btn" type="button" onclick={editPortainer}>{t('common.edit')}</button>
						<button class="btn btn-danger" type="button" disabled={pBusy} onclick={removePortainer}
							>{t('common.delete')}</button
						>
					</div>
				{:else if !pEditing}
					<button class="btn" type="button" onclick={editPortainer}
						>{t('backup.portainer.connect')}</button
					>
				{:else}
					<div class="space-y-2">
						<label for="pt-url">{t('backup.portainer.url')}</label>
						<div class="flex gap-2">
							<input id="pt-url" class="input font-mono" bind:value={pUrl} />
							<button class="btn" type="button" disabled={pBusy} onclick={probePortainer}
								>{t('backup.portainer.checkCert')}</button
							>
						</div>
						{#if pProbe}
							{#if pProbe.cert_sha256 === null}
								<p class="hint">{t('backup.portainer.http')}</p>
							{:else if pProbe.trusted}
								<p class="hint text-emerald-700 dark:text-emerald-400">
									{t('backup.portainer.trusted')}
								</p>
							{:else}
								<p class="hint">{t('backup.portainer.selfSigned')}</p>
								<p class="font-mono text-xs break-all">{pProbe.cert_sha256}</p>
								<label class="flex items-center gap-2"
									><input type="checkbox" bind:checked={pPin} />
									{t('backup.portainer.pin')}</label
								>
							{/if}
						{:else if pCert}
							<p class="hint">
								{t('backup.portainer.pinned')}: <span class="font-mono">{pCert}</span>
							</p>
						{/if}
						<label for="pt-token">{t('backup.portainer.token')}</label>
						<input
							id="pt-token"
							class="input"
							type="password"
							autocomplete="new-password"
							bind:value={pToken}
							placeholder={data.portainer ? t('backup.portainer.tokenKept') : 'ptr_…'}
						/>
						<p class="hint">{t('backup.portainer.tokenHint')}</p>
						<div class="flex flex-wrap items-end gap-2">
							<button class="btn" type="button" disabled={pBusy} onclick={loadStacks}
								>{t('backup.portainer.loadStacks')}</button
							>
							{#if pStacks.length}
								<div>
									<label for="pt-stack">{t('backup.portainer.stack')}</label>
									<select id="pt-stack" class="input" bind:value={pStack}>
										{#each pStacks as st (st.id)}<option value={st.id}>{st.name} (#{st.id})</option
											>{/each}
									</select>
								</div>
							{/if}
						</div>
						<div class="flex flex-wrap gap-2">
							<button
								class="btn btn-primary"
								type="button"
								disabled={pBusy || !pStack}
								onclick={savePortainer}>{t('backup.portainer.save')}</button
							>
							<button class="btn" type="button" onclick={() => (pEditing = false)}
								>{t('common.cancel')}</button
							>
						</div>
					</div>
				{/if}
				{#if !data.portainer}
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
			{/if}
		</div>
		{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400">{info}</p>{/if}
		<label class="flex items-center gap-2 font-medium"
			><input type="checkbox" bind:checked={data.settings.enabled} /> {t('backup.daily')}</label
		>
		<div class="grid grid-cols-1 gap-3 @xl:grid-cols-3">
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
			<div class="scroll-x">
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
