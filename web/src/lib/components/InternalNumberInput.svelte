<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';

	let {
		id,
		value = $bindable(''),
		keep = null,
		placeholder = '',
		required = false
	}: {
		id: string;
		value: string;
		/** The stored number: an older 2-digit number may be kept. */
		keep?: string | null;
		placeholder?: string;
		required?: boolean;
	} = $props();

	// New internal numbers start at 100: *1–*99 are system codes and
	// *<number> reaches internal numbers from *100.
	const pattern = $derived(value !== '' && value === keep ? '[1-9][0-9]{1,7}' : '[1-9][0-9]{2,7}');
</script>

<input
	{id}
	class="input font-mono"
	inputmode="numeric"
	{pattern}
	{placeholder}
	{required}
	title={t('numbering.hint')}
	bind:value
/>
<p class="hint">{t('numbering.hint')}</p>
