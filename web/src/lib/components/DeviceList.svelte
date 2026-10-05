<script lang="ts">
	import { onMount } from 'svelte';
	import {
		api,
		type Device,
		type DeviceCredentials,
		type DeviceKind,
		type Phone,
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
	let phones = $state<Phone[]>([]);
	let editing = $state<Device | null>(null);
	const empty = () => ({
		name: '',
		kind: 'desk' as DeviceKind,
		phone_id: '',
		account_index: '' as number | '',
		enabled: true
	});
	let form = $state(empty());

	onMount(async () => {
		if (admin) phones = await api.get<Phone[]>('/phones').catch(() => []);
	});

	function online(d: Device) {
		return registrations?.some((r) => r.user.toLowerCase() === d.sip_username.toLowerCase());
	}

	function phoneName(d: Device) {
		const p = phones.find((p) => p.id === d.phone_id);
		return p ? `${p.name} · ${t('phones.account')} ${d.account_index}` : '';
	}

	function openAdd() {
		editing = null;
		form = empty();
		addOpen = true;
	}

	function openEdit(d: Device) {
		editing = d;
		form = {
			name: d.name,
			kind: d.kind,
			phone_id: d.phone_id ?? '',
			account_index: d.account_index ?? '',
			enabled: d.enabled
		};
		addOpen = true;
	}

	async function add(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		const body = {
			...form,
			phone_id: form.phone_id || null,
			account_index: form.phone_id && form.account_index !== '' ? Number(form.account_index) : null
		};
		try {
			if (editing) {
				await api.put(`/devices/${editing.id}`, body);
			} else {
				creds = await api.post<DeviceCredentials>(`/extensions/${extensionId}/devices`, body);
				credOpen = true;
			}
			addOpen = false;
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
		{#if admin}<button class="btn btn-sm" onclick={openAdd}>{t('ext.addDevice')}</button>{/if}
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
							{d.sip_username}{d.phone_id ? ` · ${phoneName(d)}` : ''}{d.enabled
								? ''
								: ` · ${t('common.disabled')}`}
						</div>
					</div>
					<div class="space-x-1">
						<button class="btn btn-sm" onclick={() => show(d)}>{t('ext.showCredentials')}</button>
						<button class="btn btn-sm" onclick={() => reset(d)}>{t('ext.resetPassword')}</button>
						{#if admin}<button class="btn btn-sm" onclick={() => openEdit(d)}
								>{t('common.edit')}</button
							>{/if}
						{#if admin}<button class="btn btn-sm btn-danger" onclick={() => remove(d)}
								>{t('common.delete')}</button
							>{/if}
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<Modal title={editing ? t('common.edit') : t('ext.addDevice')} bind:open={addOpen}>
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
				<label for="d-phone">{t('phones.phone')} ({t('common.optional')})</label>
				<select id="d-phone" class="input" bind:value={form.phone_id}>
					<option value="">—</option>
					{#each phones as p (p.id)}<option value={p.id}>{p.name} ({p.mac})</option>{/each}
				</select>
			</div>
			<div>
				<label for="d-slot">{t('phones.account')}</label>
				<input
					id="d-slot"
					class="input"
					type="number"
					min="1"
					max="100"
					disabled={!form.phone_id}
					bind:value={form.account_index}
					placeholder={t('phones.nextFree')}
				/>
			</div>
		</div>
		<p class="hint">{t('phones.deviceHint')}</p>
		<label class="flex items-center gap-2"
			><input type="checkbox" bind:checked={form.enabled} /> {t('common.enabled')}</label
		>
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (addOpen = false)}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary">{editing ? t('common.save') : t('common.add')}</button>
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
