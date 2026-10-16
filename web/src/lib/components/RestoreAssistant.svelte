<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '#lib/api.ts';
	import { t, formatDateTime } from '#lib/i18n/index.svelte.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import FolderBrowser from '#lib/components/FolderBrowser.svelte';
	import Tabs from '#lib/components/Tabs.svelte';
	import TargetConnectionFields from '#lib/components/TargetConnectionFields.svelte';
	import { errorMessage } from '#lib/util.ts';
	import {
		defaultConfig,
		secretChange,
		type BrowseResult,
		type TargetConfig,
		type TargetSecret
	} from '#lib/backupTargets.ts';

	type Inspection = {
		manifest: {
			format: number;
			created_at: string;
			talkops_version: string;
			schema_version: number;
			volumes: string[];
			deployment: null | {
				complete: boolean;
				missing: string[];
				source: string;
				captured_at: string;
				files: string[];
				project_name: string;
				profiles: string;
				postgres_version: string;
			};
		};
		deployment_bundle: string | null;
		/** Archive in the backups volume (downloaded from a backup target). */
		archive: string | null;
	};
	let file = $state<File | null>(null);
	let token = $state('');
	let password = $state('');
	let busy = $state(false);
	let error = $state('');
	let result = $state<Inspection | null>(null);
	let reviewed = $state(false);
	let source = $state<'upload' | 'target'>('upload');
	let targetConfig = $state<TargetConfig>(defaultConfig('smb'));
	let targetSecret = $state<TargetSecret>({});
	let targetInsecure = $state(false);
	let targetPath = $state('');
	let browsing = $state(false);
	let browseKey = $state(0);
	let selected = $state<{ folder: string; name: string } | null>(null);
	let pinnedInfo = $state('');

	const prepareCommand =
		'bash recover.sh prepare --archive backup.archive --password-file backup-password --output recovered';
	const restoreSource = $derived(
		result?.archive ? `--backup ${result.archive}` : '--archive backup.archive'
	);
	const restoreCommand = $derived(
		`bash recover.sh restore ${restoreSource} --password-file backup-password -f docker-compose.yml --env-file .env --project-name talkops`
	);

	async function setupPost<T>(path: string, body: unknown): Promise<T> {
		const response = await fetch(`/api/v1/setup/restore/target/${path}`, {
			method: 'POST',
			headers: {
				'X-Requested-With': 'TalkOps',
				'X-TalkOps-Setup-Token': token,
				'Content-Type': 'application/json'
			},
			body: JSON.stringify(body),
			credentials: 'same-origin'
		});
		const data = await response.json().catch(() => ({}));
		if (!response.ok) throw new Error(data.message ?? response.statusText);
		return data as T;
	}

	function targetConnection(path: string) {
		return {
			config: $state.snapshot(targetConfig),
			path,
			allow_insecure: targetInsecure,
			secret: secretChange($state.snapshot(targetSecret))
		};
	}
	const browseTarget = (path: string) => setupPost<BrowseResult>('browse', targetConnection(path));
	function pinned(keys: string, fingerprint: string) {
		if (targetConfig.kind === 'sftp') targetConfig.host_keys = keys;
		pinnedInfo = t('targets.hostKeyPinned', { fingerprint });
	}

	async function inspectTarget(e: SubmitEvent) {
		e.preventDefault();
		if (!selected) return;
		busy = true;
		error = '';
		result = null;
		reviewed = false;
		try {
			result = await setupPost<Inspection>('inspect', {
				connection: targetConnection(selected.folder),
				name: selected.name,
				password: password || null
			});
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
			password = '';
		}
	}

	async function inspect(e: SubmitEvent) {
		e.preventDefault();
		if (!file) return;
		busy = true;
		error = '';
		result = null;
		reviewed = false;
		try {
			const body = new FormData();
			body.append('password', password);
			body.append('archive', file);
			const response = await fetch('/api/v1/setup/restore/inspect', {
				method: 'POST',
				headers: { 'X-Requested-With': 'TalkOps', 'X-TalkOps-Setup-Token': token },
				body,
				credentials: 'same-origin'
			});
			const data = await response.json().catch(() => ({}));
			if (!response.ok) throw new Error(data.message ?? response.statusText);
			result = data as Inspection;
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
			password = '';
		}
	}
	function download() {
		if (!reviewed || !result?.deployment_bundle) return;
		const bytes = Uint8Array.from(atob(result.deployment_bundle), (c) => c.charCodeAt(0));
		const url = URL.createObjectURL(new Blob([bytes], { type: 'application/gzip' }));
		const link = document.createElement('a');
		link.href = url;
		link.download = 'talkops-deployment.tar.gz';
		link.click();
		setTimeout(() => URL.revokeObjectURL(url), 1000);
	}
	async function finish() {
		error = '';
		try {
			const status = await api.get<{ needs_setup: boolean }>('/setup');
			if (status.needs_setup) throw new Error(t('restore.notFinished'));
			await goto('/login');
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<div class="space-y-4">
	<div class="card space-y-3">
		<h2>{t('restore.title')}</h2>
		<p>{t('restore.intro')}</p>
		<p class="hint">{t('restore.hostFirst')}</p>
		<p class="hint">{t('restore.limits')}</p>
		<ErrorBox {error} />
	</div>
	<div class="card space-y-3">
		<h2>{t('restore.inspect')}</h2>
		<label for="restore-token">{t('restore.token')}</label>
		<input
			id="restore-token"
			class="input"
			type="password"
			bind:value={token}
			autocomplete="off"
			minlength="32"
			required
		/>
		<p class="hint">{t('restore.tokenHint')}</p>
		<Tabs
			variant="pills"
			label={t('restore.source')}
			tabs={[
				{ id: 'upload', label: t('restore.sourceUpload') },
				{ id: 'target', label: t('restore.sourceTarget') }
			]}
			bind:value={source}
			onchange={() => (result = null)}
		/>
	</div>
	{#if source === 'target'}
		<form class="card space-y-3" onsubmit={inspectTarget}>
			<h2>{t('restore.targetTitle')}</h2>
			<p class="hint">{t('restore.targetHint')}</p>
			<TargetConnectionFields
				bind:config={targetConfig}
				bind:secret={targetSecret}
				bind:allowInsecure={targetInsecure}
				idPrefix="rt"
			/>
			<div>
				<label for="rt-path">{t('targets.path')}</label>
				<div class="flex gap-2">
					<input id="rt-path" class="input font-mono" bind:value={targetPath} />
					<button
						type="button"
						class="btn"
						disabled={token.length < 32}
						onclick={() => {
							browsing = true;
							browseKey += 1;
						}}>{t('targets.browse')}</button
					>
				</div>
			</div>
			{#if pinnedInfo}<p class="text-sm text-emerald-700 dark:text-emerald-400">
					{pinnedInfo}
				</p>{/if}
			{#if browsing}
				{#key browseKey}
					<FolderBrowser
						kind={targetConfig.kind}
						start={targetPath}
						browse={browseTarget}
						onPinned={pinned}
						onArchive={(folder, name) => {
							selected = { folder, name };
							targetPath = folder;
							result = null;
						}}
					/>
				{/key}
			{/if}
			<p class="hint">{t('restore.targetPick')}</p>
			{#if selected}<p class="font-mono text-sm break-all">{selected.name}</p>{/if}
			<label for="rt-backup-password">{t('restore.password')}</label>
			<input
				id="rt-backup-password"
				class="input"
				type="password"
				bind:value={password}
				autocomplete="off"
			/>
			<button class="btn btn-primary" disabled={busy || !selected || token.length < 32}
				>{busy ? t('common.loading') : t('restore.inspectButton')}</button
			>
		</form>
	{:else}
		<form class="card space-y-3" onsubmit={inspect}>
			<label for="restore-file">{t('restore.archive')}</label>
			<input
				id="restore-file"
				class="input"
				type="file"
				accept=".age,.gz"
				onchange={(e) => {
					file = e.currentTarget.files?.[0] ?? null;
					result = null;
				}}
				required
			/>
			<label for="restore-password">{t('restore.password')}</label>
			<input
				id="restore-password"
				class="input"
				type="password"
				bind:value={password}
				autocomplete="off"
			/>
			<button class="btn btn-primary" disabled={busy || !file}
				>{busy ? t('common.loading') : t('restore.inspectButton')}</button
			>
		</form>
	{/if}
	{#if result}
		<div class="card space-y-3">
			<h2>{t('restore.review')}</h2>
			<dl class="grid grid-cols-2 gap-2 text-sm">
				<dt>{t('restore.date')}</dt>
				<dd>{formatDateTime(result.manifest.created_at)}</dd>
				<dt>{t('status.version')}</dt>
				<dd>{result.manifest.talkops_version}</dd>
				<dt>{t('restore.schema')}</dt>
				<dd>{result.manifest.schema_version}</dd>
				<dt>{t('restore.volumes')}</dt>
				<dd>{result.manifest.volumes.join(', ')}</dd>
			</dl>
			{#if result.archive}<p class="hint">
					{t('restore.storedArchive', { name: result.archive })}
				</p>{/if}
			{#if !result.manifest.volumes.includes('recordings')}<p class="hint">
					{t('restore.noRecordings')}
				</p>{/if}
			{#if result.manifest.deployment}
				<p>
					{result.manifest.deployment.complete ? t('restore.complete') : t('restore.incomplete')}
				</p>
				{#if result.manifest.deployment.missing.length}<p class="font-mono text-sm">
						{result.manifest.deployment.missing.join(', ')}
					</p>{/if}
				<p class="hint">
					{t('restore.project')}: {result.manifest.deployment.project_name} · {t(
						'restore.profiles'
					)}: {result.manifest.deployment.profiles || t('common.none')}
				</p>
				<p class="hint">
					{t('restore.configDate')}: {formatDateTime(result.manifest.deployment.captured_at)}
				</p>
				{#if result.manifest.deployment.source === 'imported'}<p class="hint">
						{t('restore.imported')}
					</p>{/if}
			{:else}<p>{t('restore.legacy')}</p>{/if}
			{#if result.deployment_bundle}
				<label class="flex gap-2"
					><input type="checkbox" bind:checked={reviewed} />{t('restore.downloadConsent')}</label
				>
				<button class="btn" type="button" onclick={download} disabled={!reviewed}
					>{t('restore.download')}</button
				>
			{/if}
		</div>
	{/if}
	<div class="card space-y-3">
		<h2>{t('restore.hostSteps')}</h2>
		<ol class="list-decimal space-y-3 pl-5">
			<li>
				{t('restore.getTool')}
				<pre
					class="mt-2 overflow-x-auto rounded-lg bg-slate-950 p-3 text-sm text-white">docker run --rm ghcr.io/itsh-neumeier/talkops-server:latest recovery-script &gt; recover.sh</pre>
			</li>
			<li>
				{t('restore.prepareHint')}
				<pre
					class="mt-2 overflow-x-auto rounded-lg bg-slate-950 p-3 text-sm text-white">{prepareCommand}</pre>
			</li>
			<li>{t('restore.checkHost')}</li>
			<li>
				{t('restore.runHint')}
				<pre
					class="mt-2 overflow-x-auto rounded-lg bg-slate-950 p-3 text-sm text-white">{restoreCommand}</pre>
			</li>
			<li>{t('restore.portainer')}</li>
			<li>{t('restore.verify')}</li>
		</ol>
		<p class="hint">{t('restore.failure')}</p>
		<button class="btn btn-primary" type="button" onclick={finish}>{t('restore.login')}</button>
	</div>
</div>
