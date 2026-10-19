<script lang="ts">
	import { api } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import Modal from './Modal.svelte';
	import ErrorBox from './ErrorBox.svelte';
	let {
		number,
		open = $bindable(false),
		onallowed
	}: {
		number: string;
		open?: boolean;
		onallowed: (number: string) => void;
	} = $props();
	let label = $state('');
	let saving = $state(false);
	let error = $state('');
	$effect(() => {
		if (open) {
			label = '';
			error = '';
		}
	});
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (saving) return;
		saving = true;
		error = '';
		const submittedNumber = number;
		try {
			await api.post('/call-allowlist', { number: submittedNumber, label });
			onallowed(submittedNumber);
			open = false;
		} catch (err) {
			error = errorMessage(err);
		} finally {
			saving = false;
		}
	}
</script>

<Modal title={t('block.allowAdd')} bind:open>
	<form class="space-y-3" onsubmit={save} aria-busy={saving}>
		<p class="break-all font-mono font-semibold">{number}</p>
		<p class="text-sm">{t('block.allowEffect')}</p>
		<label class="block"
			>{t('block.label')}
			<input class="input mt-1" maxlength="100" bind:value={label} disabled={saving} />
		</label>
		<ErrorBox {error} />
		<div class="flex flex-wrap justify-end gap-2">
			<button type="button" class="btn" disabled={saving} onclick={() => (open = false)}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary" disabled={saving}>{t('block.allowConfirm')}</button>
		</div>
	</form>
</Modal>
