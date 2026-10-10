<script lang="ts">
	import type { PhoneNumber } from '#lib/api.ts';
	import DestinationSelect from '#lib/components/DestinationSelect.svelte';
	import { describe, targets } from '#lib/destinations.svelte.ts';
	import { saveNumber } from '#lib/numbers.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let {
		number,
		editable = false,
		onchange
	}: {
		number: PhoneNumber;
		editable?: boolean;
		onchange: () => void;
	} = $props();

	let busy = $state(false);
	let error = $state('');
	// svelte-ignore state_referenced_locally
	let type = $state(number.destination_type);
	// svelte-ignore state_referenced_locally
	let id = $state(number.destination_id);
	// svelte-ignore state_referenced_locally
	let extra = $state<string[]>([...(number.extra_extensions ?? [])]);
	/** Other extensions that can ring as well (not the main one). */
	const others = $derived(targets.extensions.filter((e) => e.id !== id));
	const extraNames = $derived(
		targets.extensions
			.filter((e) => extra.includes(e.id) && e.id !== id)
			.map((e) => e.number)
			.join(', ')
	);

	function toggleExtra(extId: string, on: boolean) {
		extra = on ? [...extra, extId] : extra.filter((x) => x !== extId);
		save();
	}

	async function save() {
		busy = true;
		error = '';
		try {
			await saveNumber(number, {
				destination_type: type,
				destination_id: id,
				extra_extensions: extra.filter((x) => x !== id)
			});
			onchange();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

{#if editable}
	<DestinationSelect
		bind:type
		bind:id
		disabled={busy}
		noneLabel={t('trunks.noDestination')}
		onchange={save}
	/>
	{#if type === 'extension' && id}
		<details class="mt-1 text-sm">
			<summary class="cursor-pointer text-slate-600 dark:text-slate-300">
				{extraNames ? t('numbers.alsoRings', { names: extraNames }) : t('numbers.addExtensions')}
			</summary>
			<div class="mt-1 grid gap-1 rounded-md border border-slate-200 p-2 dark:border-slate-700">
				{#each others as e (e.id)}
					<label class="flex items-center gap-2 font-normal">
						<input
							type="checkbox"
							checked={extra.includes(e.id)}
							disabled={busy}
							onchange={(ev) => toggleExtra(e.id, ev.currentTarget.checked)}
						/>
						<span class="font-mono">{e.number}</span>
						{e.display_name}
					</label>
				{:else}
					<span class="text-slate-500">—</span>
				{/each}
				<p class="hint">{t('numbers.mainHint')}</p>
			</div>
		</details>
	{/if}
	{#if error}<p class="text-sm text-red-600">{error}</p>{/if}
{:else}
	{describe(number.destination_type, number.destination_id)?.label ?? t('trunks.noDestination')}
	{#if number.destination_type === 'extension' && extraNames}
		<span class="text-sm text-slate-500">+ {extraNames}</span>
	{/if}
{/if}
