<script lang="ts">
	import SkeletonRows from '#lib/components/SkeletonRows.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import {
		api,
		upload,
		type Extension,
		type Firmware,
		type Phone,
		type PhoneListItem,
		type PhoneModel,
		type ProvisioningInfo
	} from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import PhoneMediaManager from '#lib/components/PhoneMediaManager.svelte';
	import PhoneSettingsDefaults from '#lib/components/PhoneSettingsDefaults.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { copy, errorMessage } from '#lib/util.ts';
	import { assignExtension } from '#lib/phoneAssign.ts';

	let phones = $state<PhoneListItem[]>([]);
	let extensions = $state<Extension[]>([]);
	/** Optional extension to put on the new phone right away. */
	let addExtension = $state('');
	/** Column filters of the list (case-insensitive substrings; model exact). */
	let filter = $state({ name: '', ext: '', model: '', mac: '', seen: '', firmware: '' });
	const has = (value: string, q: string) => value.toLowerCase().includes(q.trim().toLowerCase());
	const filtered = $derived(
		phones.filter(
			(p) =>
				has(p.name, filter.name) &&
				has(p.extensions.map((e) => `${e.number} ${e.display_name}`).join(' '), filter.ext) &&
				(!filter.model || p.model === filter.model) &&
				has(p.mac.replace(/:/g, ''), filter.mac.replace(/[:\-]/g, '')) &&
				has(`${seen(p)} ${p.last_ip ?? ''}`, filter.seen) &&
				has(p.last_firmware ?? '', filter.firmware)
		)
	);
	const filtering = $derived(Object.values(filter).some((v) => v.trim() !== ''));
	const usedModels = $derived([...new Set(phones.map((p) => p.model))]);
	let models = $state<PhoneModel[]>([]);
	let firmware = $state<Firmware[]>([]);
	let info = $state<ProvisioningInfo | null>(null);
	let error = $state('');
	let addOpen = $state(false);
	let form = $state({ mac: '', model: 't54w', name: '' });
	let fwModel = $state('t54w');
	let fwFile = $state<FileList | null>(null);
	let uploading = $state(false);

	const modelName = (id: string) => models.find((m) => m.id === id)?.name ?? id;

	async function load() {
		try {
			[phones, models] = await Promise.all([
				api.get<PhoneListItem[]>('/phones'),
				api.get<PhoneModel[]>('/phone-models')
			]);
			extensions = await api.get<Extension[]>('/extensions').catch(() => []);
			if (hasRole('admin')) firmware = await api.get<Firmware[]>('/firmware');
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	async function add(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		try {
			const phone = await api.post<Phone>('/phones', { ...form, line_keys: [] });
			if (addExtension) {
				const family = models.find((m) => m.id === phone.model)?.family;
				await assignExtension(phone, family, addExtension).catch((err) => {
					error = errorMessage(err);
				});
				addExtension = '';
			}
			addOpen = false;
			goto(`/phones/${phone.id}`);
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function showInfo(regenerate = false) {
		error = '';
		if (regenerate && !confirm(t('phones.regenerateConfirm'))) return;
		try {
			info = regenerate
				? await api.post<ProvisioningInfo>('/provisioning/regenerate')
				: await api.get<ProvisioningInfo>('/provisioning');
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function uploadFirmware(e: SubmitEvent) {
		e.preventDefault();
		const file = fwFile?.[0];
		if (!file) return;
		error = '';
		uploading = true;
		try {
			const data = new FormData();
			data.append('model', fwModel);
			data.append('file', file);
			await upload<Firmware>('/firmware', data);
			fwFile = null;
			await load();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			uploading = false;
		}
	}

	async function setActive(fw: Firmware, active: boolean) {
		try {
			await api.put(`/firmware/${fw.id}`, { active });
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function removeFirmware(fw: Firmware) {
		if (!confirm(t('common.confirmDelete', { name: fw.filename }))) return;
		try {
			await api.del(`/firmware/${fw.id}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function seen(p: Phone) {
		return p.last_seen_at ? formatDateTime(p.last_seen_at) : t('phones.neverSeen');
	}
</script>

<div class="space-y-4">
	<div class="flex flex-wrap items-center justify-between gap-2">
		<h1>{t('nav.phones')}</h1>
		{#if hasRole('admin')}
			<button class="btn btn-primary" onclick={() => (addOpen = true)}>{t('phones.new')}</button>
		{/if}
	</div>
	<ErrorBox {error} />

	<div class="card overflow-x-auto">
		{#if phones.length === 0 && net.settled}
			<p class="text-sm text-slate-500">{t('phones.none')}</p>
		{:else}
			<table class="table">
				<thead>
					<tr
						><th>{t('common.name')}</th><th>{t('phones.extensions')}</th><th>{t('phones.model')}</th
						><th>{t('phones.mac')}</th><th>{t('phones.lastSeen')}</th><th>{t('phones.firmware')}</th
						></tr
					>
					<tr class="filters">
						<th
							><input
								class="input input-sm"
								aria-label="{t('common.filter')}: {t('common.name')}"
								placeholder={t('common.filter')}
								bind:value={filter.name}
							/></th
						>
						<th
							><input
								class="input input-sm"
								aria-label="{t('common.filter')}: {t('phones.extensions')}"
								placeholder={t('common.filter')}
								bind:value={filter.ext}
							/></th
						>
						<th
							><select
								class="input input-sm"
								aria-label="{t('common.filter')}: {t('phones.model')}"
								bind:value={filter.model}
							>
								<option value="">{t('common.all')}</option>
								{#each usedModels as m (m)}<option value={m}>{modelName(m)}</option>{/each}
							</select></th
						>
						<th
							><input
								class="input input-sm font-mono"
								aria-label="{t('common.filter')}: {t('phones.mac')}"
								placeholder={t('common.filter')}
								bind:value={filter.mac}
							/></th
						>
						<th
							><input
								class="input input-sm"
								aria-label="{t('common.filter')}: {t('phones.lastSeen')}"
								placeholder={t('common.filter')}
								bind:value={filter.seen}
							/></th
						>
						<th
							><input
								class="input input-sm font-mono"
								aria-label="{t('common.filter')}: {t('phones.firmware')}"
								placeholder={t('common.filter')}
								bind:value={filter.firmware}
							/></th
						>
					</tr>
				</thead>
				<tbody>
					{#each filtered as p (p.id)}
						<tr>
							<td><a class="font-medium hover:underline" href="/phones/{p.id}">{p.name}</a></td>
							<td class="text-sm">
								{#each p.extensions as e, i (i)}<span class="whitespace-nowrap"
										><span class="font-mono">{e.number}</span> {e.display_name}</span
									>{#if i < p.extensions.length - 1}<br />{/if}{:else}<a
										class="text-slate-500 hover:underline"
										href="/phones/{p.id}">{t('phones.noneAssigned')}</a
									>{/each}
							</td>
							<td>{modelName(p.model)}</td>
							<td class="font-mono text-xs">{p.mac}</td>
							<td class="text-sm">{seen(p)}{p.last_ip ? ` · ${p.last_ip}` : ''}</td>
							<td class="font-mono text-xs">{p.last_firmware ?? '—'}</td>
						</tr>
					{:else}
						{#if !net.settled}<SkeletonRows cols={6} />{:else if filtering}<tr
								><td colspan="6" class="text-sm text-slate-500">{t('common.noMatches')}</td></tr
							>{/if}
					{/each}
				</tbody>
			</table>
		{/if}
	</div>

	{#if hasRole('admin')}
		<section class="card space-y-3">
			<h2>{t('phones.provisioning')}</h2>
			<p class="text-sm text-slate-600 dark:text-slate-300">{t('phones.provisioningHint')}</p>
			{#if info}
				<dl class="space-y-2 text-sm">
					<div>
						<dt class="text-slate-500">{t('phones.dhcp66')}</dt>
						<dd class="flex flex-wrap items-center gap-2 font-mono break-all">
							{info.url_with_credentials}
							<button class="btn btn-sm" onclick={() => copy(info!.url_with_credentials)}
								>{t('common.copy')}</button
							>
						</dd>
					</div>
					<div>
						<dt class="text-slate-500">{t('phones.serverUrl')}</dt>
						<dd class="font-mono">{info.url}</dd>
					</div>
					<div class="grid gap-2 sm:grid-cols-3">
						<div>
							<dt class="text-slate-500">{t('login.username')}</dt>
							<dd class="font-mono">{info.username}</dd>
						</div>
						<div>
							<dt class="text-slate-500">{t('login.password')}</dt>
							<dd class="font-mono">{info.password}</dd>
						</div>
						<div>
							<dt class="text-slate-500">{t('phones.adminPassword')}</dt>
							<dd class="font-mono">{info.phone_admin_password}</dd>
						</div>
					</div>
				</dl>
				<button class="btn btn-sm btn-danger" onclick={() => showInfo(true)}
					>{t('phones.regenerate')}</button
				>
			{:else}
				<button class="btn" onclick={() => showInfo()}>{t('phones.showProvisioning')}</button>
			{/if}
		</section>

		<section class="card space-y-3">
			<h2>{t('phones.firmware')}</h2>
			<p class="text-sm text-slate-600 dark:text-slate-300">{t('phones.firmwareHint')}</p>
			<form class="flex flex-wrap items-end gap-2" onsubmit={uploadFirmware}>
				<div>
					<label for="fw-model">{t('phones.model')}</label>
					<select id="fw-model" class="input" bind:value={fwModel}>
						{#each models as m (m.id)}<option value={m.id}>{m.name}</option>{/each}
					</select>
				</div>
				<div>
					<label for="fw-file">{t('phones.firmwareFile')}</label>
					<input id="fw-file" class="input" type="file" bind:files={fwFile} required />
				</div>
				<button class="btn" disabled={uploading}
					>{uploading ? t('common.loading') : t('phones.upload')}</button
				>
			</form>
			{#if firmware.length > 0}
				<div class="overflow-x-auto">
					<table class="table">
						<thead>
							<tr
								><th>{t('phones.model')}</th><th>{t('phones.firmwareFile')}</th><th
									>{t('phones.size')}</th
								><th></th></tr
							>
						</thead>
						<tbody>
							{#each firmware as fw (fw.id)}
								<tr>
									<td>{modelName(fw.model)}</td>
									<td class="font-mono text-xs" title="SHA-256 {fw.sha256}">{fw.filename}</td>
									<td class="text-sm">{(fw.size_bytes / 1048576).toFixed(1)} MB</td>
									<td class="space-x-1 text-right whitespace-nowrap">
										{#if fw.active}
											<span class="badge badge-ok">{t('phones.active')}</span>
											<button class="btn btn-sm" onclick={() => setActive(fw, false)}
												>{t('phones.deactivate')}</button
											>
										{:else}
											<button class="btn btn-sm" onclick={() => setActive(fw, true)}
												>{t('phones.activate')}</button
											>
										{/if}
										<button class="btn btn-sm btn-danger" onclick={() => removeFirmware(fw)}
											>{t('common.delete')}</button
										>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</section>

		<PhoneMediaManager />
		<PhoneSettingsDefaults />
	{/if}
</div>

<Modal title={t('phones.new')} bind:open={addOpen}>
	<form class="space-y-3" onsubmit={add}>
		<div>
			<label for="p-name">{t('common.name')}</label>
			<input id="p-name" class="input" bind:value={form.name} required placeholder="Office" />
		</div>
		<div>
			<label for="p-mac">{t('phones.mac')}</label>
			<input
				id="p-mac"
				class="input font-mono"
				bind:value={form.mac}
				required
				placeholder="80:5e:c0:12:34:56"
			/>
			<p class="hint">{t('phones.macHint')}</p>
		</div>
		<div>
			<label for="p-model">{t('phones.model')}</label>
			<select id="p-model" class="input" bind:value={form.model}>
				{#each models as m (m.id)}<option value={m.id}>{m.vendor} {m.name}</option>{/each}
			</select>
		</div>
		<div>
			<label for="p-ext">{t('phones.assignExtension')}</label>
			<select id="p-ext" class="input" bind:value={addExtension}>
				<option value="">{t('phones.assignLater')}</option>
				{#each extensions as e (e.id)}<option value={e.id}>{e.number} {e.display_name}</option
					>{/each}
			</select>
			<p class="hint">{t('phones.assignHint')}</p>
		</div>
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (addOpen = false)}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary">{t('common.add')}</button>
		</div>
	</form>
</Modal>
