<script lang="ts">
	// Side panel of a provisioned phone (UniFi Talk style): overview, network
	// data (when switched on) and the manage actions. The full phone page
	// keeps the detailed editors.
	import { api, type Extension, type PhoneDetail, type PhoneModel } from '#lib/api.ts';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { assignExtension } from '#lib/phoneAssign.ts';
	import { can } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import Collapsible from './Collapsible.svelte';
	import Combobox from './Combobox.svelte';
	import ErrorBox from './ErrorBox.svelte';
	import InfoList from './InfoList.svelte';
	import Modal from './Modal.svelte';
	import PanelAction from './PanelAction.svelte';
	import VendorBadge from './VendorBadge.svelte';
	import { modelLabel } from '#lib/vendors.ts';
	import PhoneNetworkCard from './PhoneNetworkCard.svelte';
	import SidePanel, { type PanelTab } from './SidePanel.svelte';
	import { ICONS } from './panelIcons.ts';

	let {
		phoneId,
		models,
		extensions,
		network = false,
		onchanged,
		onclose
	}: {
		phoneId: string;
		models: PhoneModel[];
		extensions: Extension[];
		/** The network data of the phones is switched on. */
		network?: boolean;
		onchanged: () => void;
		onclose: () => void;
	} = $props();

	let phone = $state<PhoneDetail | null>(null);
	let error = $state('');
	let message = $state('');
	let tab = $state('overview');
	const model = $derived(models.find((m) => m.id === phone?.model));
	const tabs = $derived<PanelTab[]>([
		{ id: 'overview', label: t('panel.overview'), icon: ICONS.overview },
		...(network ? [{ id: 'network', label: t('panel.network'), icon: ICONS.network }] : []),
		{ id: 'settings', label: t('panel.settings'), icon: ICONS.settings }
	]);

	async function load() {
		const id = phoneId;
		try {
			const value = await api.get<PhoneDetail>(`/phones/${id}`);
			if (id === phoneId) phone = value;
		} catch (err) {
			error = errorMessage(err);
		}
	}
	$effect(() => {
		void phoneId;
		phone = null;
		error = '';
		message = '';
		void load();
	});

	const assigned = $derived(
		(phone?.accounts ?? []).map((a) => `${a.extension_number} ${a.display_name}`).join(', ')
	);

	// Reassign dialog.
	let reassignOpen = $state(false);
	let target = $state('');
	let replace = $state(true);
	let resyncAfter = $state(true);
	let busy = $state(false);
	const slotsFull = $derived(!!phone && !!model && phone.accounts.length >= model.accounts);
	const options = $derived(
		extensions
			.filter((e) => !phone?.accounts.some((a) => a.extension_id === e.id))
			.map((e) => ({ value: e.id, label: `${e.number} ${e.display_name}` }))
	);
	function openReassign() {
		target = '';
		replace = true;
		resyncAfter = true;
		reassignOpen = true;
	}
	async function reassign(e: SubmitEvent) {
		e.preventDefault();
		if (!phone || !target) return;
		busy = true;
		error = '';
		try {
			if (replace || slotsFull) {
				for (const a of phone.accounts) await api.del(`/devices/${a.device_id}`);
			}
			await assignExtension(phone, model?.family, target);
			if (resyncAfter) await api.post(`/phones/${phone.id}/resync`, { reboot: false });
			reassignOpen = false;
			message = t('panel.reassigned');
			await load();
			onchanged();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	async function resync(reboot: boolean) {
		if (!phone || (reboot && !confirm(t('phones.rebootConfirm')))) return;
		error = '';
		message = '';
		try {
			const r = await api.post<{ notified: number }>(`/phones/${phone.id}/resync`, { reboot });
			message =
				r.notified === 0
					? t('phones.resyncOffline')
					: reboot
						? t('phones.rebootSent')
						: t('phones.resyncSent');
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove() {
		if (!phone || !confirm(t('common.confirmDelete', { name: phone.name }))) return;
		try {
			await api.del(`/phones/${phone.id}`);
			onchanged();
			onclose();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<SidePanel title={phone?.name ?? '…'} subtitle={model?.name ?? ''} {tabs} bind:tab {onclose}>
	<ErrorBox {error} />
	{#if message}<p class="text-sm text-emerald-700 dark:text-emerald-400" role="status">
			{message}
		</p>{/if}
	{#if phone}
		{#if tab === 'overview'}
			{#if model}
				<div class="flex justify-center rounded-xl bg-slate-50 py-4 dark:bg-slate-800/50">
					<VendorBadge vendor={model.vendor} />
				</div>
			{/if}
			<InfoList
				rows={[
					[t('phones.extensions'), assigned || t('phones.noneAssigned')],
					[t('phones.model'), model ? modelLabel(model) : phone.model],
					[t('phones.mac'), phone.mac],
					[
						t('phones.lastSeen'),
						phone.last_seen_at ? formatDateTime(phone.last_seen_at) : t('phones.neverSeen')
					],
					['IP', phone.last_ip ?? '—'],
					[t('phones.firmware'), phone.last_firmware ?? '—']
				]}
			/>
		{:else if tab === 'network'}
			<PhoneNetworkCard phoneId={phone.id} />
		{:else}
			<Collapsible title={t('panel.general')}>
				<InfoList
					rows={[
						[t('common.name'), phone.name],
						[t('phones.model'), model ? modelLabel(model) : phone.model],
						[t('phones.mac'), phone.mac]
					]}
				/>
				<PanelAction
					label={t('panel.openPage')}
					hint={t('panel.hint.openPage')}
					href="/phones/{phone.id}"
				/>
			</Collapsible>
			{#if can('phones.manage')}
				<Collapsible title={t('panel.manage')}>
					<PanelAction
						label={t('panel.reassign')}
						hint={t('panel.hint.reassign')}
						disabled={!can('extensions.manage')}
						onclick={openReassign}
					/>
					{#if model?.provisioning_capabilities?.resync !== false}
						<PanelAction
							label={t('phones.resync')}
							hint={t('panel.hint.resync')}
							onclick={() => resync(false)}
						/>
						<PanelAction
							label={t('phones.reboot')}
							hint={t('panel.hint.reboot')}
							onclick={() => resync(true)}
						/>
					{/if}
					<PanelAction
						label={t('common.delete')}
						hint={t('panel.hint.remove')}
						danger
						onclick={remove}
					/>
				</Collapsible>
			{/if}
		{/if}
	{/if}
</SidePanel>

<Modal title={t('panel.reassignTitle')} bind:open={reassignOpen}>
	{#if phone}
		<form class="space-y-4" onsubmit={reassign}>
			<p class="text-sm">
				{assigned
					? t('panel.assignedTo', { name: phone.name, list: assigned })
					: t('panel.notAssigned', { name: phone.name })}
			</p>
			<Combobox
				{options}
				bind:value={target}
				label={t('panel.assignTo')}
				placeholder={t('panel.select')}
			/>
			{#if phone.accounts.length}
				<label class="flex items-center gap-2 text-sm">
					<input type="checkbox" bind:checked={replace} disabled={slotsFull} />
					{t('panel.replace')}
				</label>
			{/if}
			{#if slotsFull || phone.accounts.length}
				<div
					class="flex gap-3 rounded-lg border border-amber-300 bg-amber-50 p-3 text-sm text-amber-900 dark:border-amber-700/60 dark:bg-amber-950/40 dark:text-amber-200"
				>
					<svg
						viewBox="0 0 24 24"
						class="mt-0.5 h-5 w-5 shrink-0"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
						aria-hidden="true"
						><path
							d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0zM12 9v4M12 17h.01"
						/></svg
					>
					<p>{slotsFull ? t('panel.slotsFull') : t('panel.reassignHint')}</p>
				</div>
			{/if}
			<label class="flex items-center gap-2 text-sm">
				<input type="checkbox" bind:checked={resyncAfter} />
				{t('panel.resyncAfter')}
			</label>
			<div class="flex justify-end gap-2">
				<button type="button" class="btn" onclick={() => (reassignOpen = false)}
					>{t('common.cancel')}</button
				>
				<button class="btn btn-primary" disabled={!target || busy}>{t('phones.assign')}</button>
			</div>
		</form>
	{/if}
</Modal>
