<script lang="ts">
	import {
		api,
		type Device,
		type DeviceCredentials,
		type DeviceKind,
		type Registration
	} from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { copy, errorMessage } from '#lib/util.ts';

	let {
		extensionId,
		devices,
		registrations = null,
		admin = false,
		onchange
	}: {
		extensionId: string;
		devices: Device[];
		registrations?: Registration[] | null;
		admin?: boolean;
		onchange: () => void;
	} = $props();

	const kinds: DeviceKind[] = ['desk', 'dect', 'softphone', 'mobile', 'door', 'other'];
	let error = $state('');
	let addOpen = $state(false);
	let credOpen = $state(false);
	let creds = $state<DeviceCredentials | null>(null);
	let form = $state({ name: '', kind: 'desk' as DeviceKind, mac: '', model: '' });

	function online(d: Device) {
		return registrations?.some((r) => r.user.toLowerCase() === d.sip_username.toLowerCase());
	}

	async function add(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		try {
			creds = await api.post<DeviceCredentials>(`/extensions/${extensionId}/devices`, {
				...form,
				mac: form.mac || null,
				model: form.model || null
			});
			addOpen = false;
			credOpen = true;
			form = { name: '', kind: 'desk', mac: '', model: '' };
			onchange();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function show(d: Device) {
		try {
			creds = await api.get<DeviceCredentials>(`/devices/${d.id}/credentials`);
			credOpen = true;
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function reset(d: Device) {
		if (!confirm(t('ext.resetConfirm'))) return;
		try {
			creds = await api.post<DeviceCredentials>(`/devices/${d.id}/reset-password`);
			credOpen = true;
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove(d: Device) {
		if (!confirm(t('common.confirmDelete', { name: d.name }))) return;
		try {
			await api.del(`/devices/${d.id}`);
			onchange();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<div class="space-y-3">
	<div class="flex items-center justify-between">
		<h2>{t('ext.devices')}</h2>
		{#if admin}<button class="btn btn-sm" onclick={() => (addOpen = true)}
				>{t('ext.addDevice')}</button
			>{/if}
	</div>
	<ErrorBox {error} />
	{#if devices.length === 0}
		<p class="text-sm text-slate-500">{t('ext.noDevices')}</p>
	{:else}
		<ul class="divide-y divide-slate-100 dark:divide-slate-800">
			{#each devices as d (d.id)}
				<li class="flex flex-wrap items-center justify-between gap-2 py-2">
					<div>
						<div class="font-medium">
							{d.name}
							<span class="text-sm font-normal text-slate-500">· {t(`kinds.${d.kind}`)}</span>
							{#if registrations}
								<span class="badge {online(d) ? 'badge-ok' : 'badge-muted'}"
									>{online(d) ? t('ext.online') : t('ext.offline')}</span
								>
							{/if}
						</div>
						<div class="font-mono text-xs text-slate-500">
							{d.sip_username}{d.mac ? ` · ${d.mac}` : ''}{d.model ? ` · ${d.model}` : ''}
						</div>
					</div>
					<div class="space-x-1">
						<button class="btn btn-sm" onclick={() => show(d)}>{t('ext.showCredentials')}</button>
						<button class="btn btn-sm" onclick={() => reset(d)}>{t('ext.resetPassword')}</button>
						{#if admin}<button class="btn btn-sm btn-danger" onclick={() => remove(d)}
								>{t('common.delete')}</button
							>{/if}
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<Modal title={t('ext.addDevice')} bind:open={addOpen}>
	<form class="space-y-3" onsubmit={add}>
		<div>
			<label for="d-name">{t('common.name')}</label>
			<input id="d-name" class="input" bind:value={form.name} required />
		</div>
		<div>
			<label for="d-kind">{t('ext.kind')}</label>
			<select id="d-kind" class="input" bind:value={form.kind}>
				{#each kinds as k (k)}<option value={k}>{t(`kinds.${k}`)}</option>{/each}
			</select>
		</div>
		<div class="grid grid-cols-2 gap-3">
			<div>
				<label for="d-mac">{t('ext.mac')} ({t('common.optional')})</label>
				<input id="d-mac" class="input font-mono" bind:value={form.mac} placeholder="80:5e:c0:…" />
			</div>
			<div>
				<label for="d-model">{t('ext.model')} ({t('common.optional')})</label>
				<input id="d-model" class="input" bind:value={form.model} placeholder="T54W" />
			</div>
		</div>
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (addOpen = false)}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary">{t('common.add')}</button>
		</div>
	</form>
</Modal>

<Modal title={t('ext.credentials')} bind:open={credOpen}>
	{#if creds}
		<dl class="space-y-3 text-sm">
			<div>
				<dt class="text-slate-500">{t('ext.sipServer')}</dt>
				<dd class="font-mono">{location.hostname}:5060</dd>
				<p class="hint">{t('ext.sipServerHint')}</p>
			</div>
			<div>
				<dt class="text-slate-500">{t('ext.sipUsername')}</dt>
				<dd class="flex items-center gap-2 font-mono">
					{creds.sip_username}
					<button class="btn btn-sm" onclick={() => copy(creds!.sip_username)}
						>{t('common.copy')}</button
					>
				</dd>
			</div>
			<div>
				<dt class="text-slate-500">{t('ext.sipPassword')}</dt>
				<dd class="flex items-center gap-2 font-mono">
					{creds.sip_password}
					<button class="btn btn-sm" onclick={() => copy(creds!.sip_password)}
						>{t('common.copy')}</button
					>
				</dd>
			</div>
		</dl>
	{/if}
</Modal>
