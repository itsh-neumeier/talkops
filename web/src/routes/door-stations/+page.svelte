<script lang="ts">
	import Skeleton from '#lib/components/Skeleton.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { onMount } from 'svelte';
	import { api, type DoorStation } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import DoorStationForm from '#lib/components/doors/DoorStationForm.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	// Setting up door stations (admin); opening, live view and events are on
	// the Door page.
	let stations = $state<DoorStation[] | null>(null);
	let error = $state('');
	let info = $state('');
	let editing = $state<DoorStation | null>(null);
	let formOpen = $state(false);
	let token = $state<{ station: DoorStation; token: string } | null>(null);
	let tokenOpen = $state(false);

	async function load() {
		try {
			stations = await api.get<DoorStation[]>('/door-stations');
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	async function test(s: DoorStation) {
		error = '';
		info = '';
		try {
			const r = await api.post<{ model: string }>(`/door-stations/${s.id}/test`, {});
			info = t('door.testOk', { model: r.model });
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function newToken(s: DoorStation) {
		if (s.has_api_token && !confirm(t('door.tokenReplace'))) return;
		try {
			const r = await api.post<{ token: string }>(`/door-stations/${s.id}/token`, {});
			token = { station: s, token: r.token };
			tokenOpen = true;
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function revokeToken(s: DoorStation) {
		try {
			await api.del(`/door-stations/${s.id}/token`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove(s: DoorStation) {
		if (!confirm(t('common.confirmDelete', { name: s.name }))) return;
		try {
			await api.del(`/door-stations/${s.id}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function edit(s: DoorStation | null) {
		editing = s;
		formOpen = true;
	}

	const hookUrl = (s: DoorStation) => `${location.origin}/hooks/door/${s.id}/open`;
</script>

<div class="space-y-4">
	<div class="flex flex-wrap items-center justify-between gap-2">
		<h1>{t('nav.doorStations')}</h1>
		<button class="btn btn-primary" onclick={() => edit(null)}>{t('door.new')}</button>
	</div>
	<p class="hint">
		{t('door.setupHint')} <a class="underline" href="/doors">{t('door.toDoorPage')}</a>
	</p>
	<ErrorBox {error} />
	{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400" role="status">{info}</p>{/if}

	{#if stations && stations.length === 0}
		<div class="card text-sm text-slate-600 dark:text-slate-300">{t('door.none')}</div>
	{/if}

	<div class="grid gap-4 md:grid-cols-2">
		{#each stations ?? [] as s (s.id)}
			<div class="card space-y-3">
				<div class="flex flex-wrap items-center justify-between gap-2">
					<div>
						<h2>{s.name}</h2>
						{#if s.model}<p class="text-xs text-slate-500">{s.model}</p>{/if}
					</div>
					<div class="flex gap-1">
						{#if s.events_enabled}
							<span class="badge {s.online ? 'badge-ok' : 'badge-bad'}"
								>{s.online ? t('door.online') : t('door.offline')}</span
							>
						{/if}
						{#if !s.enabled}<span class="badge badge-muted">{t('common.disabled')}</span>{/if}
					</div>
				</div>
				<div class="flex flex-wrap gap-1">
					<button class="btn btn-sm" onclick={() => edit(s)}>{t('common.edit')}</button>
					<button class="btn btn-sm" onclick={() => test(s)}>{t('door.test')}</button>
					<button class="btn btn-sm" onclick={() => newToken(s)}>{t('door.token')}</button>
					{#if s.has_api_token}
						<button class="btn btn-sm" onclick={() => revokeToken(s)}
							>{t('door.tokenRevoke')}</button
						>
					{/if}
					<button class="btn btn-sm btn-danger" onclick={() => remove(s)}
						>{t('common.delete')}</button
					>
				</div>
			</div>
		{:else}
			{#if !net.settled}<section class="card"><Skeleton lines={3} /></section>{/if}
		{/each}
	</div>
</div>

<Modal title={editing ? t('common.edit') : t('door.new')} bind:open={formOpen}>
	{#key editing}
		<DoorStationForm
			station={editing}
			onsaved={() => {
				formOpen = false;
				load();
			}}
			oncancel={() => (formOpen = false)}
		/>
	{/key}
</Modal>

<Modal title={t('door.token')} bind:open={tokenOpen}>
	{#if token}
		<div class="space-y-3 text-sm">
			<p>{t('door.tokenHint')}</p>
			<div>
				<span class="font-medium">URL</span>
				<code class="block rounded bg-slate-100 p-2 break-all dark:bg-slate-800"
					>POST {hookUrl(token.station)}</code
				>
			</div>
			<div>
				<span class="font-medium">Authorization</span>
				<code class="block rounded bg-slate-100 p-2 break-all dark:bg-slate-800"
					>Bearer {token.token}</code
				>
			</div>
			<div class="flex justify-end">
				<button class="btn btn-primary" onclick={() => (tokenOpen = false)}
					>{t('common.close')}</button
				>
			</div>
		</div>
	{/if}
</Modal>
