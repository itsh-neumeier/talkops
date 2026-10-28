<script lang="ts">
	import { api, type PhoneModel, type PhoneProvisioning, type PhoneParameter } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import {
		parameterCategories,
		parameterCategory,
		visibleParameters
	} from '#lib/phoneParameters.ts';
	import { errorMessage } from '#lib/util.ts';
	import ErrorBox from './ErrorBox.svelte';
	let {
		model,
		values = $bindable({}),
		disabled = false
	}: { model: PhoneModel; values: PhoneProvisioning; disabled?: boolean } = $props();
	let search = $state(''),
		configuredOnly = $state(false),
		loading = $state(false),
		error = $state('');
	let fields = $state<PhoneParameter[]>([]);
	let expanded = $state<Record<string, boolean>>({}),
		limits = $state<Record<string, number>>({});
	const settings = $derived(model.provisioning_capabilities?.settings ?? []);
	const parameterModelId = $derived(model.id);
	const parameterSlots = $derived(model.provisioning_capabilities?.native ? model.accounts : 0);
	const shown = $derived(visibleParameters(fields, search, configuredOnly, values.parameters));
	const variables = $derived(
		settings.filter(
			(s) =>
				(!configuredOnly || Object.hasOwn(values.vendor_settings ?? {}, s.key)) &&
				`${s.label} ${s.key}`.toLowerCase().includes(search.trim().toLowerCase())
		)
	);
	$effect(() => {
		const id = parameterModelId;
		const slots = parameterSlots;
		let cancelled = false;
		fields = [];
		error = '';
		loading = true;
		api
			.get<{ fields: PhoneParameter[] }>(
				`/phone-models/${encodeURIComponent(id)}/parameters${slots ? `?account_slots=${slots}` : ''}`
			)
			.then((result) => {
				if (!cancelled) fields = result.fields ?? [];
			})
			.catch((e) => {
				if (!cancelled) error = errorMessage(e);
			})
			.finally(() => {
				if (!cancelled) loading = false;
			});
		return () => {
			cancelled = true;
		};
	});
	function set(group: 'vendor_settings' | 'parameters', key: string, value: string | undefined) {
		const next = { ...values[group] };
		if (value === undefined) delete next[key];
		else next[key] = value;
		values = { ...values, [group]: next };
	}
</script>

<section class="card space-y-4">
	<h2>{t('vendor.settings')} · {model.vendor} {model.name}</h2>
	<p class="hint">{t('vendor.coverage')}</p>
	{#if /^cisco-78(?:11|21|41|61)-mpp$/.test(model.id)}<p class="hint">
			{t('vendor.cisco7800')}
		</p>{/if}
	{#if /^cisco-78(?:11|21|41|61)-enterprise$/.test(model.id)}<p class="hint">
			{t('vendor.ciscoEnterprise')}
		</p>{/if}
	<ErrorBox {error} />
	{#if loading}<p class="hint">{t('common.loading')}</p>{/if}
	<div class="flex flex-wrap items-end gap-3">
		<label class="min-w-48 flex-1" for="vendor-search"
			>{t('vendor.search')}
			<input id="vendor-search" class="input" bind:value={search} />
		</label>
		<label class="flex items-center gap-2"
			><input type="checkbox" bind:checked={configuredOnly} />{t('vendor.configuredOnly')}</label
		>
	</div>
	{#each parameterCategories as category}
		{@const categoryFields = shown.filter((f) => f.category === category)}
		{@const categoryVariables = variables.filter((s) => parameterCategory(s.key) === category)}
		{#if categoryFields.length || categoryVariables.length}
			<details
				class="rounded-lg border border-line p-3"
				open={expanded[category] || search.trim().length > 0 || configuredOnly}
				ontoggle={(e) => {
					expanded = { ...expanded, [category]: e.currentTarget.open };
				}}
			>
				<summary class="cursor-pointer font-medium"
					>{t(`vendor.category.${category}`)}
					<span class="hint">({categoryFields.length + categoryVariables.length})</span></summary
				>
				{#if expanded[category] || search.trim().length > 0 || configuredOnly}<div
						class="mt-4 grid grid-cols-1 gap-4 @lg:grid-cols-2"
					>
						{#each categoryVariables as setting (setting.key)}
							{@const saved = values.vendor_settings?.[setting.key] === 'REDACTED'}
							<div class="min-w-0">
								<label class="break-words" for="var-{setting.key}"
									>{setting.key === 'cisco_enterprise_tftp_ip'
										? t('vendor.ciscoEnterpriseIP')
										: setting.label}</label
								>
								<input
									id="var-{setting.key}"
									class="input"
									type={setting.kind === 'secret' ? 'password' : 'text'}
									autocomplete="off"
									{disabled}
									value={saved ? '' : (values.vendor_settings?.[setting.key] ?? '')}
									placeholder={saved ? t('vendor.secretSaved') : t('vendor.default')}
									oninput={(e) =>
										set('vendor_settings', setting.key, e.currentTarget.value || undefined)}
								/>
								<code class="hint break-all">{setting.key}</code>
								{#if setting.kind === 'list'}<p class="hint">{t('vendor.listHint')}</p>{/if}
								{#if !setting.copyable}<p class="hint">{t('vendor.notCopied')}</p>{/if}
								{#if Object.hasOwn(values.vendor_settings ?? {}, setting.key)}<button
										type="button"
										class="btn btn-ghost mt-1"
										{disabled}
										onclick={() => set('vendor_settings', setting.key, undefined)}
										>{t('vendor.reset')}</button
									>{/if}
							</div>
						{/each}
						{#each categoryFields.slice(0, limits[category] ?? 40) as field (field.key)}
							{@const saved = values.parameters?.[field.key] === 'REDACTED'}
							{@const configured = Object.hasOwn(values.parameters ?? {}, field.key)}
							<div class="min-w-0 space-y-1">
								<label class="block break-all text-sm" for="param-{field.key}"
									>{field.parameter}</label
								>
								{#if field.description}<p class="hint break-words">{field.description}</p>{/if}
								{#if field.managed}<p class="hint">{t('vendor.managed')}</p>
								{:else}
									<input
										id="param-{field.key}"
										class="input"
										type={field.kind === 'secret' ? 'password' : 'text'}
										autocomplete="off"
										{disabled}
										value={saved ? '' : (values.parameters?.[field.key] ?? '')}
										placeholder={saved
											? t('vendor.secretSaved')
											: (field.default ?? t('vendor.default'))}
										oninput={(e) => set('parameters', field.key, e.currentTarget.value)}
									/>
									{#if configured}<button
											type="button"
											class="btn btn-ghost"
											{disabled}
											onclick={() => set('parameters', field.key, undefined)}
											>{t('vendor.reset')}</button
										>{/if}
									{#if !field.copyable}<p class="hint">{t('vendor.notCopied')}</p>{/if}
								{/if}
								<p class="hint break-all">{field.artifact}</p>
							</div>
						{/each}
					</div>
					{#if categoryFields.length > (limits[category] ?? 40)}<button
							type="button"
							class="btn mt-3"
							onclick={() => (limits = { ...limits, [category]: (limits[category] ?? 40) + 40 })}
							>{t('vendor.showMore', {
								count: categoryFields.length - (limits[category] ?? 40)
							})}</button
						>{/if}{/if}
			</details>
		{/if}
	{/each}
	{#if !loading && !error && !fields.length && !settings.length}<p class="hint">
			{t('vendor.noFields')}
		</p>{/if}
</section>
