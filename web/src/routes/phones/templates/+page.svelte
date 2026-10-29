<script lang="ts">
	import { onMount } from 'svelte';
	import {
		api,
		getText,
		type PhoneModel,
		type ProvisioningTemplate,
		type ProvisioningRevision
	} from '#lib/api.ts';
	import PageHeader from '#lib/components/PageHeader.svelte';
	import Tabs from '#lib/components/Tabs.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import { modelLabel, sortModels, vendorName } from '#lib/vendors.ts';
	type SourceProfile = {
		model: string;
		vendor: string;
		adapted: boolean;
		files: string[];
		source_paths?: Record<string, string>;
		reason?: string | null;
	};
	let sourceRevision = $state('');
	let sources = $state<SourceProfile[]>([]),
		models = $state<PhoneModel[]>([]),
		templates = $state<ProvisioningTemplate[]>([]),
		revisions = $state<ProvisioningRevision[]>([]);
	let tab = $state('sources'),
		sourceModel = $state('t41p'),
		sourceFile = $state(''),
		sourceText = $state(''),
		error = $state(''),
		busy = $state(false),
		message = $state('');
	let selected = $state(''),
		revision = $state(0),
		filename = $state('mac.cfg'),
		files = $state<Record<string, string>>({}),
		newModel = $state('t41p'),
		name = $state(''),
		dirty = $state(false);
	const profile = $derived(sources.find((p) => p.model === sourceModel));
	const template = $derived(templates.find((p) => p.id === selected));
	async function readSource() {
		try {
			sourceText = await getText(
				`/provisioning/library/source/${encodeURIComponent(profile?.source_paths?.[sourceFile] ?? `${sourceModel}/${sourceFile}`)}`
			);
		} catch (err) {
			error = errorMessage(err);
		}
	}
	async function selectSource() {
		sourceFile = profile?.files[0] ?? '';
		await readSource();
	}
	onMount(async () => {
		if (!can('phones.manage')) return;
		try {
			const library = await api.get<{ profiles: SourceProfile[]; revision?: string }>(
				'/provisioning/library'
			);
			sources = library.profiles;
			sourceRevision = library.revision?.slice(0, 7) ?? '';
			models = await api.get<PhoneModel[]>('/phone-models');
			templates = await api.get<ProvisioningTemplate[]>('/provisioning/templates');
			await selectSource();
		} catch (err) {
			error = errorMessage(err);
		}
	});
	async function loadTemplate(id: string) {
		selected = id;
		error = '';
		revisions = await api.get<ProvisioningRevision[]>(`/provisioning/templates/${id}/revisions`);
		selectRevision(revisions[0]?.revision ?? 0);
	}
	function selectRevision(next: number) {
		revision = next;
		files = { ...(revisions.find((r) => r.revision === next)?.files ?? {}) };
		dirty = false;
	}
	async function create() {
		busy = true;
		error = '';
		try {
			const result = await api.post<ProvisioningTemplate>('/provisioning/templates', {
				name,
				model: newModel
			});
			templates = [...templates, result];
			await loadTemplate(result.id);
			message = t('prov.draftCreated');
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
	async function save() {
		busy = true;
		error = '';
		try {
			const result = await api.post<{ revision: number }>(
				`/provisioning/templates/${selected}/revisions`,
				{ files }
			);
			revisions = await api.get<ProvisioningRevision[]>(
				`/provisioning/templates/${selected}/revisions`
			);
			selectRevision(result.revision);
			message = t('prov.draftSaved');
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
	async function activate() {
		busy = true;
		error = '';
		try {
			await api.post(`/provisioning/templates/${selected}/activate`, { revision });
			templates = await api.get<ProvisioningTemplate[]>('/provisioning/templates');
			message = t('prov.activated');
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<div class="space-y-4">
	<PageHeader title={t('prov.libraryTitle')} back={{ href: '/phones', label: t('nav.phones') }} />
	<ErrorBox {error} />{#if message}<p role="status">{message}</p>{/if}
	{#if can('phones.manage')}
		<Tabs
			tabs={[
				{ id: 'sources', label: t('prov.sourceLibrary') },
				{ id: 'templates', label: t('prov.templates') }
			]}
			value={tab}
			onchange={(next) => (tab = next)}
		/>
		{#if tab === 'sources'}
			<section class="card space-y-3">
				<h2>{t('prov.sourceLibrary')}</h2>
				<p class="hint">{t('prov.sourceHint')}</p>
				<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
					<label for="source-model"
						>{t('phones.model')}<select
							id="source-model"
							aria-label={t('phones.model')}
							class="input"
							bind:value={sourceModel}
							onchange={selectSource}
							>{#each sources as source}<option value={source.model}
									>{vendorName(source.vendor)} {source.model}</option
								>{/each}</select
						></label
					><label for="source-file"
						>{t('prov.file')}<select
							id="source-file"
							aria-label={t('prov.file')}
							class="input"
							bind:value={sourceFile}
							onchange={readSource}
							>{#each profile?.files ?? [] as file}<option value={file}>{file}</option
								>{/each}</select
						></label
					>
				</div>
				<p class="hint">
					{t(profile?.adapted ? 'prov.nativeAvailable' : 'prov.referenceOnly')} · MPL-1.1 · {sourceRevision}
				</p>
				{#if profile?.reason === 'no_source_template'}<p class="hint">
						{t('vendor.sourceGap')}
					</p>{/if}
				<label for="source-code"
					>{t('prov.source')}<textarea
						id="source-code"
						class="input min-h-80 font-mono text-xs"
						value={sourceText}
						readonly
						spellcheck="false"></textarea></label
				>
			</section>
		{:else}
			<section class="card space-y-3">
				<h2>{t('prov.cloneNative')}</h2>
				<p class="hint">{t('prov.cloneHint')}</p>
				<form
					class="grid grid-cols-1 items-end gap-3 @lg:grid-cols-3"
					onsubmit={(e) => {
						e.preventDefault();
						create();
					}}
				>
					<label for="template-name"
						>{t('common.name')}<input
							id="template-name"
							class="input"
							bind:value={name}
							required
							maxlength="100"
						/></label
					><label for="template-model"
						>{t('phones.model')}<select
							id="template-model"
							aria-label={t('phones.model')}
							class="input"
							bind:value={newModel}
							>{#each sortModels(models) as model}<option value={model.id}
									>{modelLabel(model)}</option
								>{/each}</select
						></label
					><button class="btn btn-primary" disabled={busy}>{t('prov.clone')}</button>
				</form>
			</section>
			<section class="card space-y-3">
				<h2>{t('prov.templates')}</h2>
				<label for="template-select"
					>{t('prov.template')}<select
						id="template-select"
						class="input"
						value={selected}
						onchange={(e) =>
							loadTemplate(e.currentTarget.value).catch((err) => (error = errorMessage(err)))}
						><option value="" disabled>{t('prov.selectTemplate')}</option
						>{#each templates as item}<option value={item.id}>{item.name} · {item.model}</option
							>{/each}</select
					></label
				>
				{#if template}<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
						<label for="template-revision"
							>{t('prov.revision')}<select
								id="template-revision"
								aria-label={t('prov.revision')}
								class="input"
								value={revision}
								onchange={(e) => selectRevision(Number(e.currentTarget.value))}
								>{#each revisions as item}<option value={item.revision}
										>r{item.revision}{item.revision === template.active_revision
											? ` · ${t('prov.active')}`
											: ''}</option
									>{/each}</select
							></label
						><label for="template-file"
							>{t('prov.file')}<select
								id="template-file"
								aria-label={t('prov.file')}
								class="input"
								bind:value={filename}><option>mac.cfg</option><option>common.cfg</option></select
							></label
						>
					</div>
					<label for="template-code"
						>{t('prov.source')}<textarea
							id="template-code"
							class="input min-h-96 font-mono text-xs"
							value={files[filename] ?? ''}
							oninput={(e) => {
								files = { ...files, [filename]: e.currentTarget.value };
								dirty = true;
							}}
							spellcheck="false"></textarea></label
					>
					<p class="hint">{t('prov.templateHint')}</p>
					<div class="flex flex-wrap justify-end gap-2">
						<button class="btn" onclick={save} disabled={busy || !dirty}
							>{t('prov.saveRevision')}</button
						><button
							class="btn btn-primary"
							onclick={activate}
							disabled={busy || dirty || revision === template.active_revision}
							>{t('prov.activate')}</button
						>
					</div>
				{/if}
			</section>
		{/if}
	{/if}
</div>
