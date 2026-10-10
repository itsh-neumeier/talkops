<script lang="ts">
	import Skeleton from '#lib/components/Skeleton.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { onMount } from 'svelte';
	import { api, type ExtensionWithDevices } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import VoicemailInbox from '#lib/components/VoicemailInbox.svelte';
	import VoicemailSettings from '#lib/components/VoicemailSettings.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let extensions = $state<ExtensionWithDevices[] | null>(null);
	let error = $state('');

	onMount(async () => {
		try {
			extensions = await api.get<ExtensionWithDevices[]>('/me/phones');
		} catch (err) {
			error = errorMessage(err);
		}
	});
</script>

<div class="space-y-4">
	<h1>{t('nav.voicemail')}</h1>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('vm.intro')}</p>
	<ErrorBox {error} />
	{#if extensions && extensions.length === 0}
		<p class="text-sm text-slate-500">{t('me.none')}</p>
	{/if}
	{#each extensions ?? [] as ext (ext.id)}
		<section class="card space-y-4">
			<h2><span class="font-mono">{ext.number}</span> · {ext.display_name}</h2>
			<VoicemailInbox extensionId={ext.id} />
			<details>
				<summary class="cursor-pointer text-sm font-medium">{t('vm.settings')}</summary>
				<div class="pt-3">
					<VoicemailSettings extensionId={ext.id} displayName={ext.display_name} />
				</div>
			</details>
		</section>
	{:else}
		{#if !net.settled}<section class="card"><Skeleton lines={4} /></section>{/if}
	{/each}
</div>
