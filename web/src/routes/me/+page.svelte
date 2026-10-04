<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type ExtensionWithDevices } from '#lib/api.ts';
	import DeviceList from '#lib/components/DeviceList.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let phones = $state<ExtensionWithDevices[] | null>(null);
	let error = $state('');

	async function load() {
		try {
			phones = await api.get<ExtensionWithDevices[]>('/me/phones');
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);
</script>

<div class="space-y-4">
	<h1>{t('nav.myPhones')}</h1>
	<ErrorBox {error} />
	{#if phones && phones.length === 0}
		<p class="text-sm text-slate-500">{t('me.none')}</p>
	{/if}
	{#each phones ?? [] as ext (ext.id)}
		<section class="card space-y-3">
			<h2><span class="font-mono">{ext.number}</span> · {ext.display_name}</h2>
			<DeviceList extensionId={ext.id} devices={ext.devices} onchange={load} />
		</section>
	{/each}
</div>
