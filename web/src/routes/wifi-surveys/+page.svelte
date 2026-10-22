<script lang="ts">
	// Wi-Fi voice survey (ADR 0041): list and start of surveys, thresholds
	// and the read-only UniFi connection.
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api } from '#lib/api.ts';
	import EmptyRow from '#lib/components/EmptyRow.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import PageHeader from '#lib/components/PageHeader.svelte';
	import SkeletonRows from '#lib/components/SkeletonRows.svelte';
	import Tabs from '#lib/components/Tabs.svelte';
	import UnifiSettings from '#lib/components/UnifiSettings.svelte';
	import WifiSurveySettings from '#lib/components/WifiSurveySettings.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import {
		apLabel,
		band,
		clock,
		stateBadge,
		type Survey,
		type SurveyDevice,
		type UnifiState
	} from '#lib/wifiSurvey.ts';

	type Tab = 'surveys' | 'settings' | 'unifi';
	const tabs = $derived([
		{ id: 'surveys' as Tab, label: t('wifi.tabSurveys') },
		...(can('wifi_survey.manage') ? [{ id: 'settings' as Tab, label: t('wifi.tabSettings') }] : []),
		...(can('unifi.manage') ? [{ id: 'unifi' as Tab, label: t('wifi.tabUnifi') }] : [])
	]);
	let tab = $state<Tab>((page.url.searchParams.get('tab') as Tab) || 'surveys');

	let surveys = $state<Survey[] | null>(null);
	let devices = $state<SurveyDevice[]>([]);
	let error = $state('');
	let selected = $state<string[]>([]);

	// Start dialog.
	let startOpen = $state(false);
	let form = $state({ device_id: '', mode: 'originate' as 'originate' | 'self_dial', label: '' });
	let checks = $state({ vad: false, battery: false, roaming: false });
	let unifi = $state<UnifiState | null>(null);
	let unifiLoading = $state(false);
	let busy = $state(false);
	let startError = $state('');

	async function load() {
		try {
			surveys = await api.get<Survey[]>('/wifi-surveys');
			error = '';
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function openStart(device = '') {
		startError = '';
		checks = { vad: false, battery: false, roaming: false };
		const known = devices.some((d) => d.device_id === device);
		form = {
			device_id: known ? device : (devices[0]?.device_id ?? ''),
			mode: 'originate',
			label: ''
		};
		startOpen = true;
		await checkUnifi();
	}

	async function checkUnifi() {
		unifi = null;
		if (!form.device_id) return;
		unifiLoading = true;
		try {
			unifi = await api.get<UnifiState>(`/wifi-surveys/devices/${form.device_id}/unifi`);
		} catch {
			unifi = { status: 'error' };
		} finally {
			unifiLoading = false;
		}
	}

	async function start(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		try {
			const s = await api.post<Survey>('/wifi-surveys', form);
			startOpen = false;
			goto(`/wifi-surveys/${s.id}`);
		} catch (err) {
			startError = errorMessage(err);
			load();
		} finally {
			busy = false;
		}
	}

	function toggle(id: string) {
		selected = selected.includes(id)
			? selected.filter((x) => x !== id)
			: [...selected.slice(-1), id];
	}

	const device = $derived(devices.find((d) => d.device_id === form.device_id));
	const ready = $derived(checks.vad && checks.battery && checks.roaming && !!form.device_id);

	onMount(async () => {
		await load();
		try {
			devices = await api.get<SurveyDevice[]>('/wifi-surveys/devices');
		} catch (err) {
			error = errorMessage(err);
		}
		const wanted = page.url.searchParams.get('start');
		if (wanted) openStart(wanted);
	});
</script>

<div class="space-y-4">
	<PageHeader title={t('wifi.title')} intro={t('wifi.intro')}>
		{#snippet actions()}
			{#if tab === 'surveys'}
				{#if selected.length === 2}
					<a class="btn" href={`/wifi-surveys/compare?a=${selected[0]}&b=${selected[1]}`}
						>{t('wifi.compare')}</a
					>
				{/if}
				<button class="btn btn-primary" disabled={!devices.length} onclick={() => openStart()}
					>{t('wifi.start')}</button
				>
			{/if}
		{/snippet}
	</PageHeader>
	{#if tabs.length > 1}<Tabs {tabs} bind:value={tab} />{/if}

	{#if tab === 'settings'}
		<WifiSurveySettings />
	{:else if tab === 'unifi'}
		<UnifiSettings />
	{:else}
		<ErrorBox {error} />
		{#if surveys && !devices.length}
			<p class="card p-4 text-sm text-slate-600 dark:text-slate-300">{t('wifi.noDevices')}</p>
		{/if}
		<div class="card scroll-x">
			<table class="table">
				<thead>
					<tr>
						<th class="w-8"><span class="sr-only">{t('wifi.compare')}</span></th>
						<th>{t('wifi.col.started')}</th>
						<th>{t('wifi.col.device')}</th>
						<th class="col-opt">{t('wifi.col.label')}</th>
						<th>{t('wifi.col.state')}</th>
						<th class="col-opt">{t('wifi.col.duration')}</th>
						<th>{t('wifi.col.roams')}</th>
					</tr>
				</thead>
				<tbody>
					{#if !surveys}
						<SkeletonRows cols={7} />
					{:else if !surveys.length}
						<EmptyRow cols={7} text={t('wifi.empty')} />
					{:else}
						{#each surveys as s (s.id)}
							<tr>
								<td>
									<input
										type="checkbox"
										aria-label={t('wifi.selectCompare')}
										checked={selected.includes(s.id)}
										disabled={!s.summary}
										onchange={() => toggle(s.id)}
									/>
								</td>
								<td
									><a class="font-medium hover:underline" href={`/wifi-surveys/${s.id}`}
										>{formatDateTime(s.created_at)}</a
									></td
								>
								<td>{s.device_label}</td>
								<td class="col-opt">{s.label || '—'}</td>
								<td><span class={stateBadge(s.state)}>{t(`wifi.state.${s.state}`)}</span></td>
								<td class="col-opt">{s.summary ? clock(s.summary.duration_ms) : '—'}</td>
								<td>
									{#if s.summary}
										<span class="badge badge-ok">{s.summary.clean}</span>
										<span class="badge badge-warn">{s.summary.audible}</span>
										<span class="badge badge-bad">{s.summary.bad}</span>
									{:else}—{/if}
								</td>
							</tr>
						{/each}
					{/if}
				</tbody>
			</table>
		</div>
		<p class="text-sm text-slate-500 dark:text-slate-400">{t('wifi.compareHint')}</p>
	{/if}
</div>

<Modal title={t('wifi.start')} bind:open={startOpen}>
	<form class="space-y-3" onsubmit={start}>
		<ErrorBox error={startError} />
		<div>
			<label for="ws-device">{t('wifi.device')}</label>
			<select
				id="ws-device"
				class="input"
				required
				bind:value={form.device_id}
				onchange={checkUnifi}
			>
				{#each devices as d (d.device_id)}
					<option value={d.device_id} disabled={!d.enabled}
						>{d.extension_number} · {d.extension_name} · {d.device_name}{d.phone_model
							? ` (${d.phone_model})`
							: ''}</option
					>
				{/each}
			</select>
			<p class="mt-1 text-xs text-slate-500 dark:text-slate-400">
				{t('wifi.mac')}: {device?.mac ?? t('wifi.noMac')}
			</p>
		</div>
		<div
			class="rounded-lg border border-slate-200 p-3 text-sm dark:border-slate-700"
			aria-live="polite"
		>
			{#if unifiLoading}
				{t('common.loading')}
			{:else if unifi?.status === 'connected'}
				{t('wifi.unifiConnected', {
					ap: apLabel(unifi.ap_name, unifi.ap_mac),
					band: band(unifi.radio),
					channel: unifi.channel ?? '—',
					rssi: unifi.rssi ?? '—'
				})}
			{:else if unifi}
				{t(`wifi.unifiState.${unifi.status ?? 'error'}`)}
			{/if}
		</div>
		<fieldset class="space-y-1">
			<legend class="font-medium">{t('wifi.mode')}</legend>
			<label class="flex items-start gap-2 font-normal"
				><input type="radio" value="originate" bind:group={form.mode} />{t(
					'wifi.modeOriginate'
				)}</label
			>
			<label class="flex items-start gap-2 font-normal"
				><input type="radio" value="self_dial" bind:group={form.mode} />{t(
					'wifi.modeSelfDial'
				)}</label
			>
		</fieldset>
		<div>
			<label for="ws-label"
				>{t('wifi.label')} <span class="text-slate-400">({t('common.optional')})</span></label
			>
			<input
				id="ws-label"
				class="input"
				maxlength="120"
				bind:value={form.label}
				placeholder={t('wifi.labelPlaceholder')}
			/>
		</div>
		<fieldset class="space-y-1 text-sm">
			<legend class="font-medium">{t('wifi.checklist')}</legend>
			<label class="flex items-start gap-2 font-normal"
				><input type="checkbox" bind:checked={checks.vad} />{t('wifi.checkVad')}</label
			>
			<label class="flex items-start gap-2 font-normal"
				><input type="checkbox" bind:checked={checks.battery} />{t('wifi.checkBattery')}</label
			>
			<label class="flex items-start gap-2 font-normal"
				><input type="checkbox" bind:checked={checks.roaming} />{t('wifi.checkRoaming')}</label
			>
		</fieldset>
		<p class="text-xs text-slate-500 dark:text-slate-400">{t('wifi.privacyHint')}</p>
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (startOpen = false)}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary" disabled={busy || !ready}>{t('wifi.startNow')}</button>
		</div>
	</form>
</Modal>
