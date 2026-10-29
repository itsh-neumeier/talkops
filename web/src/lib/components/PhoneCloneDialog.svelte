<script lang="ts">
	import { goto } from '$app/navigation';
	import { api, type Phone, type PhoneModel, type Extension } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import { modelLabel } from '#lib/vendors.ts';
	import ErrorBox from './ErrorBox.svelte';
	import Modal from './Modal.svelte';
	let {
		open = $bindable(false),
		sourceId = $bindable(''),
		phones,
		models,
		extensions
	}: {
		open?: boolean;
		sourceId?: string;
		phones: Pick<Phone, 'id' | 'name' | 'model'>[];
		models: PhoneModel[];
		extensions: Extension[];
	} = $props();
	type Preview = {
		source_revision: string;
		configuration: Pick<
			Phone,
			| 'model'
			| 'line_keys'
			| 'settings'
			| 'provisioning'
			| 'phonebook_sections'
			| 'ringtone_id'
			| 'wallpaper_id'
		>;
		excluded: string[];
		http_exception: boolean;
	};
	let preview = $state<Preview | null>(null),
		error = $state(''),
		busy = $state(false);
	let name = $state(''),
		mac = $state(''),
		assignments = $state<Record<string, string>>({});
	const baseModel = $derived(models.find((m) => m.id === preview?.configuration.model));
	const model = $derived(
		baseModel
			? {
					...baseModel,
					accounts: preview?.configuration.provisioning?.account_slots ?? baseModel.accounts
				}
			: undefined
	);
	$effect(() => {
		const id = sourceId;
		if (!open || !id) {
			preview = null;
			return;
		}
		let cancelled = false;
		preview = null;
		error = '';
		name = '';
		mac = '';
		assignments = {};
		api
			.post<Preview>(`/phones/${id}/clone-preview`, {})
			.then((result) => {
				if (!cancelled) preview = result;
			})
			.catch((err) => {
				if (!cancelled) error = errorMessage(err);
			});
		return () => {
			cancelled = true;
		};
	});
	async function create(event: SubmitEvent) {
		event.preventDefault();
		if (!preview || busy) return;
		busy = true;
		error = '';
		try {
			const phone = await api.post<Phone>(`/phones/${sourceId}/clone`, {
				name,
				mac,
				source_revision: preview.source_revision,
				assignments: Object.entries(assignments)
					.filter(([, extension]) => extension)
					.map(([slot, extension_id]) => ({ account_index: Number(slot), extension_id }))
			});
			open = false;
			await goto(`/phones/${phone.id}`);
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<Modal title={t('phoneclone.title')} bind:open>
	<form class="space-y-3" onsubmit={create}>
		<ErrorBox {error} />
		<label for="clone-source">{t('phoneclone.source')}</label>
		<select id="clone-source" class="input" bind:value={sourceId} disabled={busy} required>
			<option value="">{t('phoneclone.select')}</option>
			{#each phones as phone (phone.id)}<option value={phone.id}
					>{phone.name} · {models.find((m) => m.id === phone.model)?.name ?? phone.model}</option
				>{/each}
		</select>
		{#if preview}
			<p class="hint">{t('phoneclone.hint')}</p>
			<p>{model ? modelLabel(model) : preview.configuration.model}</p>
			<details>
				<summary>{t('phoneclone.copied')}</summary>
				<pre class="max-h-48 overflow-auto text-xs">{JSON.stringify(
						preview.configuration,
						null,
						2
					)}</pre>
			</details>
			<details>
				<summary>{t('phoneclone.excluded')}</summary>
				<ul class="list-inside list-disc text-sm">
					{#each preview.excluded as field}<li>{field}</li>{/each}
				</ul>
			</details>
			{#if preview.http_exception}<p role="status" class="hint">{t('phoneclone.http')}</p>{/if}
			<label for="clone-name">{t('common.name')}</label>
			<input id="clone-name" class="input" bind:value={name} required disabled={busy} />
			<label for="clone-mac"
				>{t(model?.family === 'app' ? 'vendor.identityToken' : 'phones.mac')}</label
			>
			<input
				id="clone-mac"
				class="input font-mono"
				bind:value={mac}
				required={model?.family !== 'app'}
				disabled={busy}
			/>
			{#if model?.family === 'app'}<p class="hint">{t('vendor.identityHint')}</p>{/if}
			{#if can('extensions.manage') && model}
				<fieldset class="space-y-2">
					<legend>{t('phoneclone.assignments')}</legend>
					<p class="hint">{t('phoneclone.accountHint')}</p>
					{#each Array.from({ length: model.accounts }, (_, i) => i + 1) as slot (slot)}
						<label for="clone-account-{slot}">{t('phones.account')} {slot}</label>
						<select
							id="clone-account-{slot}"
							class="input"
							bind:value={assignments[slot]}
							disabled={busy}
						>
							<option value="">{t('phones.assignLater')}</option>
							{#each extensions as extension (extension.id)}<option value={extension.id}
									>{extension.number} {extension.display_name}</option
								>{/each}
						</select>
					{/each}
				</fieldset>
			{/if}
		{/if}
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (open = false)} disabled={busy}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary" disabled={!preview || busy}>{t('phoneclone.create')}</button>
		</div>
	</form>
</Modal>
