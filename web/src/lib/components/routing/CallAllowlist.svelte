<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import ErrorBox from '../ErrorBox.svelte';
	import AllowNumberDialog from '../AllowNumberDialog.svelte';
	import Modal from '../Modal.svelte';
	import Skeleton from '../Skeleton.svelte';
	type Entry = { id: string; number: string; label: string; created_at: string };
	let entries = $state<Entry[]>([]);
	let number = $state('');
	let open = $state(false);
	let deleting = $state<Entry | null>(null);
	let confirmDelete = $state(false);
	let busy = $state(false);
	let loading = $state(true);
	let error = $state('');
	async function load() {
		try {
			entries = await api.get<Entry[]>('/call-allowlist');
			error = '';
		} catch (err) {
			error = errorMessage(err);
		} finally {
			loading = false;
		}
	}
	onMount(load);
	async function remove() {
		if (!deleting || busy) return;
		busy = true;
		try {
			await api.del(`/call-allowlist/${deleting.id}`);
			confirmDelete = false;
			await load();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<section class="card space-y-3">
	<h2>{t('block.allowlist')}</h2>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('block.allowEffect')}</p>
	<ErrorBox {error} />
	<form
		class="flex flex-wrap items-end gap-2"
		onsubmit={(e) => {
			e.preventDefault();
			open = true;
		}}
	>
		<label class="min-w-0 flex-1"
			>{t('block.allowNumber')}
			<input
				class="input mt-1 font-mono"
				bind:value={number}
				required
				pattern={'\\+[1-9][0-9]{2,14}'}
				maxlength="16"
				placeholder="+4930123456"
			/>
		</label>
		<button class="btn btn-primary">{t('block.allowAdd')}</button>
	</form>
	{#if loading}<Skeleton lines={3} />
	{:else if entries.length}
		<div class="scroll-x">
			<table class="w-full text-left text-sm">
				<thead
					><tr
						><th>{t('block.allowNumber')}</th><th>{t('block.label')}</th><th>{t('block.added')}</th
						><th><span class="sr-only">{t('common.delete')}</span></th></tr
					></thead
				>
				<tbody
					>{#each entries as entry (entry.id)}
						<tr
							><td class="font-mono">{entry.number}</td><td>{entry.label}</td><td
								>{formatDateTime(entry.created_at)}</td
							><td
								><button
									class="btn btn-danger btn-sm"
									onclick={() => {
										deleting = entry;
										error = '';
										confirmDelete = true;
									}}>{t('common.delete')}</button
								></td
							></tr
						>
					{/each}</tbody
				>
			</table>
		</div>
	{:else if !error}<p class="hint">{t('block.allowEmpty')}</p>{/if}
	<AllowNumberDialog
		{number}
		bind:open
		onallowed={() => {
			number = '';
			load();
		}}
	/>
	<Modal title={t('block.allowRemove')} bind:open={confirmDelete}>
		<p class="break-all font-mono">{deleting?.number}</p>
		<p class="my-3 text-sm">{t('block.allowRemoveEffect')}</p>
		<ErrorBox {error} />
		<div class="flex justify-end gap-2">
			<button class="btn" disabled={busy} onclick={() => (confirmDelete = false)}
				>{t('common.cancel')}</button
			><button class="btn btn-danger" disabled={busy} onclick={remove}>{t('common.delete')}</button>
		</div>
	</Modal>
</section>
