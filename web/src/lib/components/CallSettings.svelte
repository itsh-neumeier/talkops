<script lang="ts">
	import { api, type Extension } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let { extension, onchange }: { extension: Extension; onchange?: () => void } = $props();

	// svelte-ignore state_referenced_locally
	let dnd = $state(extension.dnd);
	// svelte-ignore state_referenced_locally
	let forward = $state(extension.forward_all ?? '');
	let error = $state('');
	let saved = $state(false);

	async function save(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		saved = false;
		try {
			await api.put<Extension>(`/extensions/${extension.id}/call-settings`, {
				dnd,
				forward_all: forward.replace(/[\s\-/()]/g, '') || null
			});
			saved = true;
			onchange?.();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<form class="space-y-3" onsubmit={save}>
	<h2>{t('calls.settings')}</h2>
	<ErrorBox {error} />
	<label class="flex items-center gap-2">
		<input type="checkbox" bind:checked={dnd} />
		{t('calls.dnd')}
	</label>
	<div>
		<label for="cs-fwd-{extension.id}">{t('calls.forwardAll')}</label>
		<input
			id="cs-fwd-{extension.id}"
			class="input font-mono"
			bind:value={forward}
			placeholder={t('calls.forwardPlaceholder')}
			inputmode="tel"
		/>
		<p class="hint">{t('calls.featureCodes')}</p>
	</div>
	<div class="flex items-center justify-end gap-3">
		{#if saved}<span class="text-sm text-emerald-700 dark:text-emerald-400"
				>{t('common.saved')}</span
			>{/if}
		<button class="btn btn-primary">{t('common.save')}</button>
	</div>
</form>
