<script lang="ts">
	import { api, type PhoneNumber } from '#lib/api.ts';
	import DestinationSelect from '#lib/components/DestinationSelect.svelte';
	import { describe } from '#lib/destinations.svelte.ts';
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

	async function save() {
		busy = true;
		error = '';
		try {
			await api.put(`/numbers/${number.id}`, {
				trunk_id: number.trunk_id,
				account_id: number.account_id,
				e164: number.e164,
				label: number.label,
				destination_type: type,
				destination_id: id,
				enabled: number.enabled
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
	{#if error}<p class="text-sm text-red-600">{error}</p>{/if}
{:else}
	{describe(number.destination_type, number.destination_id)?.label ?? t('trunks.noDestination')}
{/if}
