<script lang="ts">
	import type { PhoneModel, PhoneProvisioning, ProvisioningKey } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	let {
		model,
		values = $bindable({}),
		disabled = false
	}: { model: PhoneModel; values: PhoneProvisioning; disabled?: boolean } = $props();
	const groups = $derived(model.provisioning_capabilities?.key_groups ?? []);
	const nativeFunctions = $derived(model.provisioning_capabilities?.key_functions ?? []);
	function firstFunction(group: string) {
		return nativeFunctions.find((f) => !f.groups?.length || f.groups.includes(group))?.uuid;
	}
	function add() {
		const group = groups[0];
		if (!group) return;
		const used = new Set((values.keys ?? []).filter((k) => k.group === group).map((k) => k.key));
		let slot = 1;
		while (used.has(slot) && slot <= 512) slot++;
		values = {
			...values,
			keys: [
				...(values.keys ?? []),
				{
					group,
					native_function: firstFunction(group),
					key: slot,
					type: 'none',
					value: '',
					label: '',
					account: 1,
					history_type: 0,
					phonebook: 0
				}
			]
		};
	}
	function update(index: number, change: Partial<ProvisioningKey>) {
		values = {
			...values,
			keys: (values.keys ?? []).map((key, i) => (i === index ? { ...key, ...change } : key))
		};
	}
</script>

{#if model.provisioning_capabilities?.native && groups.length}
	<section class="card space-y-3">
		<h2>{t('vendor.category.keys')}</h2>
		<p class="hint">{t('vendor.keySlotsHint')}</p>
		{#each values.keys ?? [] as key, index (index)}
			<fieldset class="grid grid-cols-1 gap-3 rounded-lg border border-line p-3 @lg:grid-cols-3">
				<label
					>{t('vendor.keyGroup')}<select
						class="input"
						{disabled}
						value={key.group}
						onchange={(e) =>
							update(index, {
								group: e.currentTarget.value,
								native_function: firstFunction(e.currentTarget.value),
								type: 'none'
							})}
						>{#each groups as group}<option value={group}>{group}</option>{/each}</select
					></label
				>
				<label
					>{t('vendor.keySlot')}<input
						class="input"
						type="number"
						min="1"
						max="512"
						{disabled}
						value={key.key}
						oninput={(e) => update(index, { key: Number(e.currentTarget.value) })}
					/></label
				>
				<label
					>{t('phones.keyType')}<select
						class="input"
						{disabled}
						value={key.native_function ?? key.type}
						onchange={(e) =>
							update(
								index,
								nativeFunctions.some((f) => f.uuid === e.currentTarget.value)
									? { native_function: e.currentTarget.value, type: 'none' }
									: {
											native_function: undefined,
											type: e.currentTarget.value as ProvisioningKey['type']
										}
							)}
					>
						{#if !key.native_function}<option value={key.type}>{t(`keytype.${key.type}`)}</option
							>{/if}
						{#each nativeFunctions.filter((f) => !f.groups?.length || f.groups.includes(key.group)) as f}<option
								value={f.uuid}
								>{f.type.replaceAll('_', ' ')}{f.subtype ? ` (${f.subtype})` : ''}</option
							>{/each}
					</select></label
				>
				{#if key.native_function}<p class="hint @lg:col-span-3">
						{nativeFunctions.find((f) => f.uuid === key.native_function)?.description ?? ''}
					</p>{/if}
				<label
					>{t('phones.account')}<input
						class="input"
						type="number"
						min="1"
						max={model.accounts}
						{disabled}
						value={key.account}
						oninput={(e) => update(index, { account: Number(e.currentTarget.value) })}
					/></label
				>
				<label
					>{t('phones.keyLabel')}<input
						class="input"
						maxlength="32"
						{disabled}
						value={key.label}
						oninput={(e) => update(index, { label: e.currentTarget.value })}
					/></label
				>
				<label
					>{t('phonekeys.number')}<input
						class="input"
						{disabled}
						value={key.value}
						oninput={(e) => update(index, { value: e.currentTarget.value })}
					/></label
				>
				<button
					type="button"
					class="btn"
					{disabled}
					onclick={() =>
						(values = { ...values, keys: (values.keys ?? []).filter((_, i) => i !== index) })}
					>{t('vendor.removeKey')}</button
				>
			</fieldset>
		{/each}
		<button type="button" class="btn" {disabled} onclick={add}>{t('vendor.addKey')}</button>
	</section>
{/if}
