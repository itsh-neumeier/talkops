<script lang="ts">
	import type { DestinationType } from '#lib/api.ts';
	import { targetGroups } from '#lib/destinations.svelte.ts';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';

	let {
		type = $bindable(),
		id = $bindable(),
		inputId = undefined,
		exclude = null,
		disabled = false,
		noneLabel = null,
		onchange
	}: {
		type: DestinationType;
		id: string | null;
		inputId?: string;
		exclude?: string | null;
		disabled?: boolean;
		noneLabel?: string | null;
		onchange?: () => void;
	} = $props();

	const value = $derived(type === 'none' || !id ? '' : `${type}:${id}`);

	function select(v: string) {
		if (!v) {
			type = 'none';
			id = null;
		} else {
			const [t2, i] = v.split(':');
			type = t2 as DestinationType;
			id = i;
		}
		onchange?.();
	}
</script>

<select
	id={inputId}
	class="input"
	{value}
	{disabled}
	onchange={(e) => select(e.currentTarget.value)}
>
	<option value="">{noneLabel ?? t('dest.none')}</option>
	{#each targetGroups() as g (g.key)}
		<optgroup label={t(g.key as MessageKey)}>
			{#each g.options.filter((o) => o.id !== exclude) as o (o.type + o.id)}
				<option value="{o.type}:{o.id}">{o.label}</option>
			{/each}
		</optgroup>
	{/each}
</select>
