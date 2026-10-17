<script lang="ts">
	import DeviceLocaleFields from './DeviceLocaleFields.svelte';
	import { onMount } from 'svelte';
	import { api, type ProvisioningRequest } from '#lib/api.ts';
	import { t, formatDateTime } from '#lib/i18n/index.svelte.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { errorMessage } from '#lib/util.ts';
	let endpoint = $state(''),
		sipHost = $state(''),
		error = $state(''),
		saved = $state(false),
		busy = $state(false);
	let requests = $state<ProvisioningRequest[]>([]);
	let locale = $state({ language: '', time_zone: '', ntp_server: '' });
	async function refresh() {
		requests = await api.get<ProvisioningRequest[]>('/provisioning/requests');
	}
	onMount(async () => {
		try {
			const options = await api.get<{
				endpoint: string;
				sip_host: string;
				language?: string;
				time_zone?: string;
				ntp_server?: string;
			}>('/provisioning/options');
			endpoint = options.endpoint;
			sipHost = options.sip_host;
			locale = {
				language: options.language ?? '',
				time_zone: options.time_zone ?? '',
				ntp_server: options.ntp_server ?? ''
			};
			await refresh();
		} catch (err) {
			error = errorMessage(err);
		}
	});
	async function save() {
		busy = true;
		error = '';
		saved = false;
		try {
			await api.put('/provisioning/options', { endpoint, sip_host: sipHost, ...locale });
			saved = true;
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<section class="card space-y-3">
	<h2>{t('prov.transportTitle')}</h2>
	<p class="hint">{t('prov.transportHint')}</p>
	<ErrorBox {error} />
	<form
		class="space-y-3"
		onsubmit={(e) => {
			e.preventDefault();
			save();
		}}
	>
		<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
			<label for="provision-endpoint"
				>{t('prov.endpoint')}<input
					id="provision-endpoint"
					class="input font-mono"
					placeholder="https://pbx.example/provisioning"
					bind:value={endpoint}
				/></label
			><label for="provision-sip"
				>{t('prov.sipHost')}<input id="provision-sip" class="input" bind:value={sipHost} /></label
			>
		</div>
		<p class="hint">{t('prov.httpHint')}</p>
		<DeviceLocaleFields prefix="provision-defaults" bind:values={locale} disabled={busy} />
		<div class="flex flex-wrap items-center justify-end gap-2">
			{#if saved}<span role="status">{t('phoneset.saved')}</span>{/if}<button
				class="btn btn-primary"
				disabled={busy}>{t('common.save')}</button
			>
		</div>
	</form>
	<div class="flex flex-wrap items-center justify-between gap-2">
		<h3>{t('prov.requests')}</h3>
		<button
			type="button"
			class="btn btn-sm"
			onclick={() => refresh().catch((err) => (error = errorMessage(err)))}
			>{t('common.refresh')}</button
		>
	</div>
	<p class="hint">{t('prov.resyncHint')}</p>
	<div class="scroll-x">
		<table class="table">
			<thead
				><tr
					><th>{t('prov.time')}</th><th>{t('phones.mac')}</th><th>{t('prov.file')}</th><th
						>{t('prov.transport')}</th
					><th>{t('prov.result')}</th></tr
				></thead
			><tbody
				>{#each requests as request}<tr
						><td>{formatDateTime(request.created_at)}</td><td class="font-mono"
							>{request.mac ?? '—'}</td
						><td class="break-all font-mono">{request.filename}</td><td
							>{request.scheme.toUpperCase()}</td
						><td>{request.status}</td></tr
					>{:else}<tr><td colspan="5">{t('prov.noRequests')}</td></tr>{/each}</tbody
			>
		</table>
	</div>
</section>
