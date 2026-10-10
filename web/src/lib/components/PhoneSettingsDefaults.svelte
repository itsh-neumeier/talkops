<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type PhoneSettingsView } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import PhoneSettingsEditor from '#lib/components/PhoneSettingsEditor.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let view = $state<PhoneSettingsView | null>(null);
	let values = $state<Record<string, string>>({});
	let error = $state('');
	let saved = $state(false);
	let busy = $state(false);

	onMount(async () => {
		try {
			view = await api.get<PhoneSettingsView>('/phone-settings');
			values = { ...view.values };
		} catch (err) {
			error = errorMessage(err);
		}
	});

	async function save() {
		error = '';
		saved = false;
		busy = true;
		try {
			view = await api.put<PhoneSettingsView>('/phone-settings', { values });
			values = { ...view.values };
			saved = true;
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

{#if view && view.catalog.length}
	<section class="card space-y-3">
		<h2>{t('phoneset.title')}</h2>
		<p class="hint">{t('phoneset.allHint')}</p>
		<ErrorBox {error} />
		<PhoneSettingsEditor catalog={view.catalog} bind:values idPrefix="psd" />
		<div class="flex items-center justify-end gap-3">
			{#if saved}<span class="text-sm text-emerald-700">{t('phoneset.saved')}</span>{/if}
			<button class="btn btn-primary" disabled={busy} onclick={save}>{t('common.save')}</button>
		</div>
	</section>
{:else if error}
	<ErrorBox {error} />
{/if}
