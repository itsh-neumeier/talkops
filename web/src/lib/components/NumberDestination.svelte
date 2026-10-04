<script lang="ts">
	import { api, type Extension, type PhoneNumber } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';

	let {
		number,
		extensions,
		editable = false,
		onchange
	}: {
		number: PhoneNumber;
		extensions: Extension[];
		editable?: boolean;
		onchange: () => void;
	} = $props();

	let busy = $state(false);
	const current = $derived(
		number.destination_type === 'extension' ? (number.destination_id ?? '') : ''
	);

	async function change(value: string) {
		busy = true;
		try {
			await api.put(`/numbers/${number.id}`, {
				trunk_id: number.trunk_id,
				account_id: number.account_id,
				e164: number.e164,
				label: number.label,
				destination_type: value ? 'extension' : 'none',
				destination_id: value || null,
				enabled: number.enabled
			});
			onchange();
		} finally {
			busy = false;
		}
	}
</script>

{#if editable}
	<select
		class="input mt-0 py-1"
		value={current}
		disabled={busy}
		onchange={(e) => change(e.currentTarget.value)}
		aria-label={t('trunks.destination')}
	>
		<option value="">{t('trunks.noDestination')}</option>
		{#each extensions as e (e.id)}<option value={e.id}>{e.number} {e.display_name}</option>{/each}
	</select>
{:else}
	{@const ext = extensions.find((e) => e.id === current)}
	{ext ? `${ext.number} ${ext.display_name}` : t('trunks.noDestination')}
{/if}
