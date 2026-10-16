<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import FolderBrowser from '#lib/components/FolderBrowser.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import TargetConnectionFields from '#lib/components/TargetConnectionFields.svelte';
	import { formatDateTime, t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import {
		defaultConfig,
		describe,
		secretChange,
		type BrowseResult,
		type StorageOverview,
		type StorageTarget,
		type TargetConfig,
		type TargetSecret,
		type TestReport
	} from '#lib/backupTargets.ts';

	type Draft = {
		id?: string;
		name: string;
		config: TargetConfig;
		path: string;
		allow_insecure: boolean;
		bwlimit_kbps: number | null;
		has_secret: boolean;
		used: number;
	};

	let data = $state<StorageOverview | null>(null);
	let error = $state('');
	let info = $state('');
	let testingId = $state('');

	let editing = $state(false);
	let draft = $state<Draft>(newDraft());
	let secret = $state<TargetSecret>({});
	let formError = $state('');
	let formInfo = $state('');
	let browsing = $state(false);
	let testing = $state(false);
	let saving = $state(false);

	function newDraft(): Draft {
		return {
			name: '',
			config: defaultConfig('smb'),
			path: '',
			allow_insecure: false,
			bwlimit_kbps: null,
			has_secret: false,
			used: 0
		};
	}

	async function load() {
		try {
			data = await api.get<StorageOverview>('/storage-targets');
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	function edit(x?: StorageTarget) {
		draft = x
			? {
					id: x.id,
					name: x.name,
					config: structuredClone($state.snapshot(x.config)),
					path: x.path,
					allow_insecure: x.allow_insecure,
					bwlimit_kbps: x.bwlimit_kbps,
					has_secret: x.has_secret,
					used: x.used_by.length
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
			const report = await api.post<TestReport>('/storage-targets/test', connection());
			formInfo = t('targets.testOk');
			if (report.host_keys && report.fingerprint) pinned(report.host_keys, report.fingerprint);
		} catch (err) {
			formError = errorMessage(err);
		} finally {
			testing = false;
		}
	}

	/** Tests a saved target; the server records the result. */
	async function testSaved(x: StorageTarget) {
		testingId = x.id;
		error = '';
		info = '';
		try {
			await api.post(`/storage-targets/${x.id}/test`, {});
			await load();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			testingId = '';
		}
	}

	const browseFn = (path: string) =>
		api.post<BrowseResult>('/storage-targets/browse', { ...connection(), path });
	const mkdirFn = (parent: string, name: string) =>
		api.post<void>('/storage-targets/folders', {
			connection: { ...connection(), path: parent },
			name
		});

	async function save(e: SubmitEvent) {
		e.preventDefault();
		saving = true;
		formError = '';
		try {
			const body = {
				name: draft.name,
				config: $state.snapshot(draft.config),
				path: draft.path,
				allow_insecure: draft.allow_insecure,
				bwlimit_kbps: draft.bwlimit_kbps ? Number(draft.bwlimit_kbps) : null,
				secret: secretChange($state.snapshot(secret))
			};
			if (draft.id) await api.put(`/storage-targets/${draft.id}`, body);
			else await api.post('/storage-targets', body);
			editing = false;
			info = t('common.saved');
			await load();
		} catch (err) {
			formError = errorMessage(err);
		} finally {
			saving = false;
		}
	}

	async function remove(x: StorageTarget) {
		if (!confirm(t('storage.confirmDelete', { name: x.name }))) return;
		error = '';
		try {
			await api.del(`/storage-targets/${x.id}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function testState(x: StorageTarget): { cls: string; text: string } {
		if (x.last_test_ok === true) return { cls: 'badge-ok', text: t('storage.tested') };
		if (x.last_test_ok === false) return { cls: 'badge-bad', text: t('storage.testFailed') };
		return { cls: 'badge-muted', text: t('storage.untested') };
	}

	const kindLabel = (kind: string) => t(`targets.kind.${kind}` as MessageKey);
	const purposeLabel = (purpose: string) => t(`storage.purpose.${purpose}` as MessageKey);
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
			<h2>{t('storage.title')}</h2>
			<button class="btn btn-primary" type="button" onclick={() => edit()}
				>{t('storage.add')}</button
			>
		</div>
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('storage.hint')}</p>
		<ErrorBox {error} />
		{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400">{info}</p>{/if}
		{#if data.targets.length}
			<div class="scroll-x">
				<table class="table">
					<thead>
						<tr>
							<th>{t('storage.target')}</th>
							<th class="col-opt">{t('storage.usedBy')}</th>
							<th>{t('storage.connection')}</th>
							<th><span class="sr-only">{t('backup.actions')}</span></th>
						</tr>
					</thead>
					<tbody>
						{#each data.targets as x (x.id)}
							{@const st = testState(x)}
							<tr>
								<td>
									<span class="font-medium">{x.name}</span>
									<span class="block text-xs text-slate-500">{kindLabel(x.config.kind)}</span>
									<span class="block font-mono text-xs break-all text-slate-500"
										>{describe(x.config, x.path)}</span
									>
								</td>
								<td class="col-opt text-sm">
									{#each x.used_by as u (u.id)}
										<span class="block"
											><span class="badge badge-muted">{purposeLabel(u.purpose)}</span>
											{u.name}</span
										>
									{:else}
										<span class="text-xs text-slate-500">{t('storage.unused')}</span>
									{/each}
								</td>
								<td class="text-sm">
									<span class="badge {st.cls}">{st.text}</span>
									{#if x.last_test_at}<span class="block text-xs text-slate-500"
											>{t('storage.lastTest')}: {formatDateTime(x.last_test_at)}</span
										>{/if}
									{#if x.last_test_error}<span
											class="block text-xs break-all text-red-700 dark:text-red-400"
											>{x.last_test_error}</span
										>{/if}
								</td>
								<td class="text-right">
									<div class="flex flex-col items-end gap-1 sm:flex-row sm:justify-end">
										<button
											class="btn btn-sm"
											type="button"
											disabled={testingId === x.id}
											onclick={() => testSaved(x)}
											>{testingId === x.id ? t('common.loading') : t('targets.test')}</button
										>
										<button class="btn btn-sm" type="button" onclick={() => edit(x)}
											>{t('common.edit')}</button
										>
										<button
											class="btn btn-sm btn-danger"
											type="button"
											disabled={x.used_by.length > 0}
											title={x.used_by.length ? t('storage.inUse') : undefined}
											onclick={() => remove(x)}>{t('common.delete')}</button
										>
									</div>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else}
			<p class="hint">{t('storage.none')}</p>
		{/if}
	</div>
{/if}

<Modal title={draft.id ? t('storage.edit') : t('storage.add')} bind:open={editing}>
	<form class="space-y-4" onsubmit={save}>
		<ErrorBox error={formError} />
		{#if draft.used}<p class="hint text-amber-700 dark:text-amber-400">
				{t('storage.sharedHint', { n: draft.used })}
			</p>{/if}
		<div>
			<label for="st-name">{t('common.name')}</label>
			<input id="st-name" class="input" bind:value={draft.name} required maxlength="100" />
		</div>
		<TargetConnectionFields
			bind:config={draft.config}
			bind:secret
			bind:allowInsecure={draft.allow_insecure}
			hasSecret={draft.has_secret}
			roots={data?.roots ?? []}
			idPrefix="st"
		/>
		<div>
			<label for="st-path">{t('targets.path')}</label>
			<div class="flex gap-2">
				<input id="st-path" class="input min-w-0 font-mono" bind:value={draft.path} />
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
		<div>
			<label for="st-bw">{t('targets.bwlimit')}</label>
			<input
				id="st-bw"
				class="input"
				type="number"
				min="1"
				bind:value={draft.bwlimit_kbps}
				placeholder={t('targets.unlimited')}
			/>
		</div>
		<div class="flex flex-wrap items-center gap-2">
			<button type="button" class="btn" disabled={testing} onclick={test}
				>{testing ? t('common.loading') : t('targets.test')}</button
			>
			{#if formInfo}<span class="text-sm text-emerald-700 dark:text-emerald-400">{formInfo}</span
				>{/if}
		</div>
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (editing = false)}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary" disabled={saving}>{t('common.save')}</button>
		</div>
	</form>
</Modal>
