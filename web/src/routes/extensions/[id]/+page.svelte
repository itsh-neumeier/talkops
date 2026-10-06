<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import {
		api,
		type ExtensionWithDevices,
		type LiveStatus,
		type PhoneNumber,
		type User
	} from '#lib/api.ts';
	import CallSettings from '#lib/components/CallSettings.svelte';
	import DeviceList from '#lib/components/DeviceList.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import ExtensionForm from '#lib/components/ExtensionForm.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import VoicemailSettings from '#lib/components/VoicemailSettings.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let ext = $state<ExtensionWithDevices | null>(null);
	let users = $state<User[]>([]);
	let numbers = $state<PhoneNumber[]>([]);
	let live = $state<LiveStatus | null>(null);
	let error = $state('');
	let editOpen = $state(false);
	const id = $derived(page.params.id);

	async function load() {
		try {
			ext = await api.get<ExtensionWithDevices>(`/extensions/${id}`);
			if (hasRole('operator')) {
				[users, numbers, live] = await Promise.all([
					api.get<User[]>('/users'),
					api.get<PhoneNumber[]>('/numbers'),
					api.get<LiveStatus>('/telephony/status')
				]);
			}
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	async function remove() {
		if (!ext || !confirm(t('common.confirmDelete', { name: `${ext.number} ${ext.display_name}` })))
			return;
		try {
			await api.del(`/extensions/${ext.id}`);
			goto('/extensions');
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<div class="space-y-4">
	<a class="text-sm text-slate-500 hover:underline" href="/extensions">← {t('common.back')}</a>
	<ErrorBox {error} />
	{#if ext}
		<div class="flex flex-wrap items-center justify-between gap-2">
			<h1><span class="font-mono">{ext.number}</span> · {ext.display_name}</h1>
			{#if hasRole('admin')}
				<div class="space-x-1">
					<button class="btn" onclick={() => (editOpen = true)}>{t('common.edit')}</button>
					<button class="btn btn-danger" onclick={remove}>{t('common.delete')}</button>
				</div>
			{/if}
		</div>
		<section class="card">
			<CallSettings extension={ext} onchange={load} />
		</section>
		{#if hasRole('admin')}
			<section class="card">
				<VoicemailSettings extensionId={ext.id} displayName={ext.display_name} />
			</section>
		{/if}
		<section class="card">
			<DeviceList
				extensionId={ext.id}
				devices={ext.devices}
				registrations={live?.registrations ?? null}
				admin={hasRole('admin')}
				onchange={load}
			/>
		</section>
		<Modal title={t('common.edit')} bind:open={editOpen}>
			<ExtensionForm
				extension={ext}
				{users}
				{numbers}
				onsaved={async () => {
					editOpen = false;
					await load();
				}}
				oncancel={() => (editOpen = false)}
			/>
		</Modal>
	{/if}
</div>
