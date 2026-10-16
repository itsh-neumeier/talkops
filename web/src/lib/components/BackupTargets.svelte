<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import FolderBrowser from '#lib/components/FolderBrowser.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import TargetConnectionFields from '#lib/components/TargetConnectionFields.svelte';
	import { formatDateTime, i18n, t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import {
		defaultConfig,
		defaultRetention,
		defaultSchedule,
		describe,
		hasDay,
		runTimes,
		secretChange,
		toggleDay,
		type BackupTarget,
		type BrowseResult,
		type Entry,
		type Retention,
		type Schedule,
		type TargetConfig,
		type TargetSecret,
		type TargetsOverview,
		type TestReport
	} from '#lib/backupTargets.ts';

	type Draft = {
		id?: string;
		name: string;
		enabled: boolean;
		config: TargetConfig;
		path: string;
		allow_insecure: boolean;
		include_recordings: boolean;
		schedule: Schedule;
		retention: Retention;
		bwlimit_kbps: number | null;
		has_secret: boolean;
	};

	let data = $state<TargetsOverview | null>(null);
	let error = $state('');
	let info = $state('');
	let timer: ReturnType<typeof setTimeout> | undefined;

	let editing = $state(false);
	let draft = $state<Draft>(newDraft());
	let secret = $state<TargetSecret>({});
	let formError = $state('');
	let formInfo = $state('');
	let browsing = $state(false);
	let testing = $state(false);
	let saving = $state(false);

	let archivesOpen = $state(false);
	let archivesFor = $state<BackupTarget | null>(null);
	let archives = $state<Entry[]>([]);
	let archivesError = $state('');
	let archivesLoading = $state(false);

	function newDraft(): Draft {
		return {
			name: '',
			enabled: true,
			config: defaultConfig('smb'),
			path: '',
			allow_insecure: false,
			include_recordings: true,
			schedule: defaultSchedule(),
			retention: defaultRetention(),
			bwlimit_kbps: null,
			has_secret: false
		};
	}

	async function load() {
		try {
			data = await api.get<TargetsOverview>('/backup-targets');
		} catch (err) {
			error = errorMessage(err);
		}
		clearTimeout(timer);
		if (data?.targets.some((x) => x.activity && !x.activity.done)) timer = setTimeout(load, 2000);
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

	function scheduleText(s: Schedule): string {
		if (s.weekdays === 0) return t('targets.manualOnly');
		const days =
			s.weekdays === 127
				? t('targets.everyDay')
				: weekdays.filter((_, i) => hasDay(s.weekdays, i)).join(', ');
		return `${days} · ${runTimes(s).join(', ')}`;
	}

	function retentionText(r: Retention): string {
		const parts = [t('targets.ret.last', { n: r.keep_last })];
		if (r.keep_daily) parts.push(t('targets.ret.daily', { n: r.keep_daily }));
		if (r.keep_weekly) parts.push(t('targets.ret.weekly', { n: r.keep_weekly }));
		if (r.keep_monthly) parts.push(t('targets.ret.monthly', { n: r.keep_monthly }));
		if (r.keep_yearly) parts.push(t('targets.ret.yearly', { n: r.keep_yearly }));
		return parts.join(', ');
	}

	function status(x: BackupTarget): { cls: string; text: string } {
		if (x.activity && !x.activity.done)
			return {
				cls: 'badge-warn',
				text: x.activity.action === 'fetch' ? t('targets.fetching') : t('targets.running')
			};
		if (!x.enabled) return { cls: 'badge-muted', text: t('common.disabled') };
		if (x.last_error) return { cls: 'badge-bad', text: t('targets.failed') };
		if (x.last_success_at) return { cls: 'badge-ok', text: t('targets.ok') };
		return { cls: 'badge-muted', text: t('targets.never') };
	}

	function edit(x?: BackupTarget) {
		draft = x
			? {
					id: x.id,
					name: x.name,
					enabled: x.enabled,
					config: structuredClone($state.snapshot(x.config)),
					path: x.path,
					allow_insecure: x.allow_insecure,
					include_recordings: x.include_recordings,
					schedule: { ...x.schedule },
					retention: { ...x.retention },
					bwlimit_kbps: x.bwlimit_kbps,
					has_secret: x.has_secret
				}
			: newDraft();
		secret = {};
		formError = '';
		formInfo = '';
		browsing = false;
		editing = true;
	}

	function connection() {
		return {
			id: draft.id,
			config: $state.snapshot(draft.config),
			path: draft.path,
			allow_insecure: draft.allow_insecure,
			secret: secretChange($state.snapshot(secret))
		};
	}

	function pinned(keys: string, fingerprint: string) {
		if (draft.config.kind === 'sftp') draft.config.host_keys = keys;
		formInfo = t('targets.hostKeyPinned', { fingerprint });
	}

	async function test() {
		testing = true;
		formError = '';
		formInfo = '';
		try {
			const report = await api.post<TestReport>('/backup-targets/test', connection());
			formInfo = t('targets.testOk');
			if (report.host_keys && report.fingerprint) pinned(report.host_keys, report.fingerprint);
		} catch (err) {
			formError = errorMessage(err);
		} finally {
			testing = false;
		}
	}

	const browseFn = (path: string) =>
		api.post<BrowseResult>('/backup-targets/browse', { ...connection(), path });
	const mkdirFn = (parent: string, name: string) =>
		api.post<void>('/backup-targets/folders', {
			connection: { ...connection(), path: parent },
			name
		});

	function setTime(value: string) {
		const [h, m] = value.split(':').map(Number);
		if (Number.isFinite(h) && Number.isFinite(m)) {
			draft.schedule.hour = h;
			draft.schedule.minute = m;
		}
	}

	async function save(e: SubmitEvent) {
		e.preventDefault();
		saving = true;
		formError = '';
		try {
			const body = {
				name: draft.name,
				enabled: draft.enabled,
				config: $state.snapshot(draft.config),
				path: draft.path,
				allow_insecure: draft.allow_insecure,
				include_recordings: draft.include_recordings,
				schedule: $state.snapshot(draft.schedule),
				retention: $state.snapshot(draft.retention),
				bwlimit_kbps: draft.bwlimit_kbps ? Number(draft.bwlimit_kbps) : null,
				secret: secretChange($state.snapshot(secret))
			};
			if (draft.id) await api.put(`/backup-targets/${draft.id}`, body);
			else await api.post('/backup-targets', body);
			editing = false;
			info = t('common.saved');
			await load();
		} catch (err) {
			formError = errorMessage(err);
		} finally {
			saving = false;
		}
	}

	async function remove(x: BackupTarget) {
		if (!confirm(t('targets.confirmDelete', { name: x.name }))) return;
		error = '';
		try {
			await api.del(`/backup-targets/${x.id}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function runNow(x: BackupTarget) {
		error = '';
		info = '';
		try {
			await api.post(`/backup-targets/${x.id}/run`, {});
			info = t('targets.started');
			setTimeout(load, 500);
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function openArchives(x: BackupTarget) {
		archivesFor = x;
		archivesOpen = true;
		archives = [];
		archivesError = '';
		archivesLoading = true;
		try {
			archives = await api.get<Entry[]>(`/backup-targets/${x.id}/archives`);
		} catch (err) {
			archivesError = errorMessage(err);
		} finally {
			archivesLoading = false;
		}
	}

	async function fetchArchive(name: string) {
		if (!archivesFor) return;
		archivesError = '';
		try {
			await api.post(
				`/backup-targets/${archivesFor.id}/archives/${encodeURIComponent(name)}/fetch`,
				{}
			);
			archivesOpen = false;
			info = t('targets.fetchStarted', { name });
			setTimeout(load, 500);
		} catch (err) {
			archivesError = errorMessage(err);
		}
	}

	async function deleteArchive(name: string) {
		if (!archivesFor || !confirm(t('common.confirmDelete', { name }))) return;
		archivesError = '';
		try {
			await api.del(`/backup-targets/${archivesFor.id}/archives/${encodeURIComponent(name)}`);
			archives = archives.filter((a) => a.name !== name);
		} catch (err) {
			archivesError = errorMessage(err);
		}
	}

	function size(bytes: number): string {
		if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(1)} GB`;
		return `${(bytes / 1048576).toFixed(1)} MB`;
	}
	const kindLabel = (kind: string) => t(`targets.kind.${kind}` as MessageKey);
	const pathHint = $derived(
		draft.config.kind === 's3'
			? t('targets.pathS3')
			: draft.config.kind === 'smb'
				? t('targets.pathSmb')
				: draft.config.kind === 'local'
					? t('targets.pathLocal')
					: t('targets.pathOther')
	);
</script>

{#if data}
	<div class="card space-y-4">
		<div class="flex flex-wrap items-center justify-between gap-2">
			<h2>{t('targets.title')}</h2>
			<button class="btn btn-primary" type="button" onclick={() => edit()}
				>{t('targets.add')}</button
			>
		</div>
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('targets.hint')}</p>
		{#if !data.encryption_configured}<p class="hint text-amber-700 dark:text-amber-400">
				{t('targets.encryptionMissing')}
			</p>{/if}
		<ErrorBox {error} />
		{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400">{info}</p>{/if}
		{#if data.targets.length}
			<div class="scroll-x">
				<table class="table">
					<thead>
						<tr>
							<th>{t('targets.target')}</th>
							<th class="col-opt">{t('targets.schedule')}</th>
							<th>{t('targets.status')}</th>
							<th><span class="sr-only">{t('backup.actions')}</span></th>
						</tr>
					</thead>
					<tbody>
						{#each data.targets as x (x.id)}
							{@const st = status(x)}
							<tr>
								<td>
									<span class="font-medium">{x.name}</span>
									<span class="block text-xs text-slate-500">{kindLabel(x.config.kind)}</span>
									<span class="block font-mono text-xs break-all text-slate-500"
										>{describe(x.config, x.path)}</span
									>
								</td>
								<td class="col-opt text-sm">
									{scheduleText(x.schedule)}
									<span class="block text-xs text-slate-500">{retentionText(x.retention)}</span>
									{#if x.next_run_at}<span class="block text-xs text-slate-500"
											>{t('targets.next')}: {formatDateTime(x.next_run_at)}</span
										>{/if}
								</td>
								<td class="text-sm">
									<span class="badge {st.cls}">{st.text}</span>
									<span class="block text-xs text-slate-500"
										>{t('targets.lastSuccess')}: {formatDateTime(x.last_success_at)}</span
									>
									{#if x.last_error}<span class="block text-xs text-red-700 dark:text-red-400"
											>{x.last_error}</span
										>{/if}
									{#if x.activity?.done && x.activity.action === 'fetch'}<span
											class="block text-xs {x.activity.error
												? 'text-red-700 dark:text-red-400'
												: 'text-emerald-700 dark:text-emerald-400'}"
											>{x.activity.error ??
												t('targets.fetched', { name: x.activity.file ?? '' })}</span
										>{/if}
								</td>
								<td class="text-right">
									<div class="flex flex-col items-end gap-1 sm:flex-row sm:justify-end">
										<button
											class="btn btn-sm"
											type="button"
											disabled={!!x.activity && !x.activity.done}
											onclick={() => runNow(x)}>{t('targets.runNow')}</button
										>
										<button class="btn btn-sm" type="button" onclick={() => openArchives(x)}
											>{t('targets.archives')}</button
										>
										<button class="btn btn-sm" type="button" onclick={() => edit(x)}
											>{t('common.edit')}</button
										>
										<button class="btn btn-sm btn-danger" type="button" onclick={() => remove(x)}
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
			<p class="hint">{t('targets.none')}</p>
		{/if}
	</div>
{/if}

<Modal title={draft.id ? t('targets.edit') : t('targets.add')} bind:open={editing}>
	<form class="space-y-4" onsubmit={save}>
		<ErrorBox error={formError} />
		<div class="grid grid-cols-1 gap-3 @lg:grid-cols-3">
			<div class="@lg:col-span-2">
				<label for="bt-name">{t('common.name')}</label>
				<input id="bt-name" class="input" bind:value={draft.name} required maxlength="100" />
			</div>
			<label class="flex items-center gap-2 self-end pb-2"
				><input type="checkbox" bind:checked={draft.enabled} /> {t('common.enabled')}</label
			>
		</div>
		<TargetConnectionFields
			bind:config={draft.config}
			bind:secret
			bind:allowInsecure={draft.allow_insecure}
			hasSecret={draft.has_secret}
			roots={data?.roots ?? []}
			idPrefix="bt"
		/>
		<div>
			<label for="bt-path">{t('targets.path')}</label>
			<div class="flex gap-2">
				<input id="bt-path" class="input font-mono" bind:value={draft.path} />
				<button type="button" class="btn" onclick={() => (browsing = !browsing)}
					>{t('targets.browse')}</button
				>
			</div>
			<p class="hint">{pathHint}</p>
		</div>
		{#if browsing}
			<FolderBrowser
				kind={draft.config.kind}
				start={draft.path}
				roots={data?.roots ?? []}
				browse={browseFn}
				mkdir={mkdirFn}
				onPinned={pinned}
				onSelect={(p) => {
					draft.path = p;
					browsing = false;
				}}
			/>
		{/if}
		<div class="flex flex-wrap items-center gap-2">
			<button type="button" class="btn" disabled={testing} onclick={test}
				>{testing ? t('common.loading') : t('targets.test')}</button
			>
			{#if formInfo}<span class="text-sm text-emerald-700 dark:text-emerald-400">{formInfo}</span
				>{/if}
		</div>

		<fieldset class="space-y-2 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
			<legend class="px-1 text-sm font-medium">{t('targets.schedule')}</legend>
			<div class="flex flex-wrap gap-1" role="group" aria-label={t('targets.weekdays')}>
				{#each weekdays as day, i (i)}
					<button
						type="button"
						class="btn btn-sm {hasDay(draft.schedule.weekdays, i) ? 'btn-primary' : ''}"
						aria-pressed={hasDay(draft.schedule.weekdays, i)}
						onclick={() => (draft.schedule.weekdays = toggleDay(draft.schedule.weekdays, i))}
						>{day}</button
					>
				{/each}
			</div>
			<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
				<div>
					<label for="bt-time">{t('targets.firstRun')}</label>
					<input
						id="bt-time"
						class="input"
						type="time"
						value={`${String(draft.schedule.hour).padStart(2, '0')}:${String(draft.schedule.minute).padStart(2, '0')}`}
						onchange={(e) => setTime(e.currentTarget.value)}
					/>
				</div>
				<div>
					<label for="bt-every">{t('targets.repeat')}</label>
					<select id="bt-every" class="input" bind:value={draft.schedule.every_hours}>
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
			<legend class="px-1 text-sm font-medium">{t('targets.retention')}</legend>
			<p class="hint">{t('targets.retentionHint')}</p>
			<div class="grid grid-cols-2 gap-3 @lg:grid-cols-5">
				{#each [['keep_last', 'targets.keepLast', 1, 1000], ['keep_daily', 'targets.keepDaily', 0, 3660], ['keep_weekly', 'targets.keepWeekly', 0, 520], ['keep_monthly', 'targets.keepMonthly', 0, 1200], ['keep_yearly', 'targets.keepYearly', 0, 100]] as const as [field, label, min, max] (field)}
					<div>
						<label for={`bt-${field}`}>{t(label)}</label>
						<input
							id={`bt-${field}`}
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
			<p class="hint">{retentionText(draft.retention)}</p>
		</fieldset>

		<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
			<label class="flex items-center gap-2"
				><input type="checkbox" bind:checked={draft.include_recordings} />
				{t('backup.recordings')}</label
			>
			<div>
				<label for="bt-bw">{t('targets.bwlimit')}</label>
				<input
					id="bt-bw"
					class="input"
					type="number"
					min="1"
					bind:value={draft.bwlimit_kbps}
					placeholder={t('targets.unlimited')}
				/>
			</div>
		</div>
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (editing = false)}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary" disabled={saving}>{t('common.save')}</button>
		</div>
	</form>
</Modal>

<Modal title={t('targets.archivesOf', { name: archivesFor?.name ?? '' })} bind:open={archivesOpen}>
	<div class="space-y-3">
		<ErrorBox error={archivesError} />
		<p class="hint">{t('targets.archivesHint')}</p>
		{#if archivesLoading}
			<p class="hint">{t('common.loading')}</p>
		{:else if archives.length}
			<ul class="divide-y divide-slate-100 dark:divide-slate-800">
				{#each archives as a (a.name)}
					<li class="flex flex-wrap items-center justify-between gap-2 py-2">
						<span class="min-w-0">
							<span class="block font-mono text-sm break-all">{a.name}</span>
							<span class="text-xs text-slate-500">{size(a.size)}</span>
						</span>
						<span class="flex gap-1">
							<button class="btn btn-sm" type="button" onclick={() => fetchArchive(a.name)}
								>{t('targets.fetch')}</button
							>
							<button
								class="btn btn-sm btn-danger"
								type="button"
								onclick={() => deleteArchive(a.name)}>{t('common.delete')}</button
							>
						</span>
					</li>
				{/each}
			</ul>
		{:else}
			<p class="hint">{t('targets.noArchives')}</p>
		{/if}
	</div>
</Modal>
