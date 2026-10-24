<script lang="ts">
	// Outbound dialing permissions (ADR 0045): profiles, rules, default
	// profile and a number test.
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import CountryFlag from '#lib/components/CountryFlag.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import {
		countryOptions,
		missingPremium,
		normalizeValue,
		profileInput,
		sortRules,
		upsertRule,
		type DialTestResult,
		type DialingProfile,
		type DialingRule,
		type PremiumRule,
		type RuleAction,
		type RuleKind
	} from '#lib/dialing.ts';
	import { i18n, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	type Draft = {
		id: string | null;
		name: string;
		description: string;
		base_mode: RuleAction;
		sip_addresses: boolean;
		rules: DialingRule[];
	};

	let profiles = $state<DialingProfile[]>([]);
	let premium = $state<PremiumRule[]>([]);
	let error = $state('');
	let open = $state(false);
	let draft = $state<Draft>(empty());
	let dialogError = $state('');

	// New rule in the dialog.
	let kind = $state<RuleKind>('prefix');
	let value = $state('');
	let action = $state<RuleAction>('deny');
	let label = $state('');
	let valueError = $state('');

	// Number test.
	let testNumber = $state('');
	let testProfile = $state('');
	let result = $state<DialTestResult | null>(null);
	let testError = $state('');

	const countries = $derived(countryOptions(i18n.locale));

	function empty(): Draft {
		return {
			id: null,
			name: '',
			description: '',
			base_mode: 'allow',
			sip_addresses: true,
			rules: []
		};
	}

	async function load() {
		try {
			[profiles, premium] = await Promise.all([
				api.get<DialingProfile[]>('/dialing-profiles'),
				api.get<PremiumRule[]>('/dialing-profiles/premium-rules')
			]);
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	function edit(p: DialingProfile | null) {
		draft = p
			? {
					id: p.id,
					name: p.name,
					description: p.description,
					base_mode: p.base_mode,
					sip_addresses: p.sip_addresses,
					rules: sortRules(p.rules.map((r) => ({ ...r })))
				}
			: empty();
		dialogError = '';
		value = '';
		label = '';
		valueError = '';
		open = true;
	}

	function addRule(e: SubmitEvent) {
		e.preventDefault();
		const normalized = normalizeValue(kind, value);
		if (!normalized) {
			valueError = t('dialing.valueInvalid');
			return;
		}
		draft.rules = upsertRule(draft.rules, { kind, value: normalized, action, label: label.trim() });
		value = '';
		label = '';
		valueError = '';
	}

	function removeRule(rule: DialingRule) {
		draft.rules = draft.rules.filter((r) => r !== rule);
	}

	function addPremium() {
		draft.rules = sortRules([...draft.rules, ...missingPremium(draft.rules, premium)]);
	}

	async function save() {
		dialogError = '';
		try {
			const body = profileInput(draft);
			if (draft.id) await api.put(`/dialing-profiles/${draft.id}`, body);
			else await api.post('/dialing-profiles', body);
			open = false;
			await load();
		} catch (err) {
			dialogError = errorMessage(err);
		}
	}

	async function makeDefault(p: DialingProfile) {
		error = '';
		try {
			await api.put(`/dialing-profiles/${p.id}/default`, {});
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove(p: DialingProfile) {
		if (!confirm(t('common.confirmDelete', { name: p.name }))) return;
		error = '';
		try {
			await api.del(`/dialing-profiles/${p.id}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function test(e: SubmitEvent) {
		e.preventDefault();
		result = null;
		testError = '';
		try {
			result = await api.post<DialTestResult>('/dialing-profiles/test', {
				number: testNumber,
				profile_id: testProfile || null
			});
		} catch (err) {
			testError = errorMessage(err);
		}
	}

	function ruleText(r: DialingRule): string {
		return `${t(`dialing.kind.${r.kind}`)} ${r.value}`;
	}
</script>

<div class="space-y-4">
	<ErrorBox {error} />
	<section class="card space-y-3">
		<div class="flex flex-wrap items-center justify-between gap-2">
			<h2>{t('dialing.profiles')}</h2>
			<button class="btn btn-primary" onclick={() => edit(null)}>{t('dialing.new')}</button>
		</div>
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('dialing.intro')}</p>
		<ul class="divide-y divide-slate-200 dark:divide-slate-700" data-testid="dialing-profiles">
			{#each profiles as p (p.id)}
				<li class="flex flex-wrap items-center gap-x-3 gap-y-2 py-2">
					<div class="min-w-0 flex-1">
						<div class="flex flex-wrap items-center gap-2">
							<span class="font-medium">{p.name}</span>
							{#if p.is_default}<span class="badge badge-ok">{t('dialing.default')}</span>{/if}
							<span class={p.base_mode === 'allow' ? 'badge badge-muted' : 'badge badge-warn'}
								>{t('dialing.baseMode')}: {t(`dialing.base.${p.base_mode}`)}</span
							>
							<span class="badge badge-muted"
								>{t('dialing.ruleCount', { count: p.rules.length })}</span
							>
						</div>
						{#if p.description}<p class="text-sm text-slate-600 dark:text-slate-300">
								{p.description}
							</p>{/if}
						<p class="text-xs text-slate-500 dark:text-slate-400">
							{t('dialing.extensions', { count: p.extensions })}
						</p>
					</div>
					<div class="flex flex-wrap gap-2">
						<button class="btn btn-sm" onclick={() => edit(p)}>{t('common.edit')}</button>
						{#if !p.is_default}
							<button class="btn btn-sm" onclick={() => makeDefault(p)}
								>{t('dialing.makeDefault')}</button
							>
							<button class="btn btn-danger btn-sm" onclick={() => remove(p)}
								>{t('common.delete')}</button
							>
						{/if}
					</div>
				</li>
			{/each}
		</ul>
	</section>

	<form class="card space-y-3" onsubmit={test}>
		<h2>{t('dialing.test')}</h2>
		<p class="hint">{t('dialing.testHint')}</p>
		<div class="grid grid-cols-1 gap-3 @lg:grid-cols-3">
			<div>
				<label for="d-test">{t('dialing.testNumber')}</label>
				<input
					id="d-test"
					class="input font-mono"
					placeholder="0900 1234567"
					bind:value={testNumber}
					required
				/>
			</div>
			<div>
				<label for="d-test-profile">{t('dialing.testProfile')}</label>
				<select id="d-test-profile" class="input" bind:value={testProfile}>
					<option value="">{t('dialing.testDefault')}</option>
					{#each profiles as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
				</select>
			</div>
			<div class="flex items-end">
				<button class="btn">{t('dialing.testRun')}</button>
			</div>
		</div>
		<ErrorBox error={testError} />
		{#if result}
			<div
				class="space-y-1 rounded-lg border border-slate-200 p-3 text-sm dark:border-slate-700"
				data-testid="dialing-test"
			>
				<p class="flex flex-wrap items-center gap-2">
					<span class={result.allowed ? 'badge badge-ok' : 'badge badge-bad'}
						>{result.allowed ? t('dialing.result.allowed') : t('dialing.result.blocked')}</span
					>
					{#if result.number}<span class="font-mono">{result.number}</span>{/if}
					{#if result.country}<CountryFlag country={result.country} />
					{:else if result.number}<span class="text-slate-500 dark:text-slate-400"
							>{t('dialing.result.noCountry')}</span
						>{/if}
				</p>
				{#if result.class === 'emergency'}<p>{t('dialing.result.emergency')}</p>
				{:else if result.class === 'invalid'}<p>{t('dialing.result.invalid')}</p>
				{:else if !result.profile}<p>{t('dialing.result.noProfile')}</p>
				{:else}
					<p>{t('dialing.result.profile', { name: result.profile.name })}</p>
					<p>
						{result.rule
							? t('dialing.result.rule', {
									rule: `${ruleText(result.rule)} → ${t(`dialing.action.${result.rule.action}`)}${result.rule.label ? ` (${result.rule.label})` : ''}`
								})
							: t('dialing.result.base')}
					</p>
				{/if}
			</div>
		{/if}
	</form>
</div>

<Modal title={draft.id ? t('dialing.edit') : t('dialing.new')} bind:open>
	<div class="space-y-3">
		<ErrorBox error={dialogError} />
		<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
			<div>
				<label for="d-name">{t('dialing.name')}</label>
				<input id="d-name" class="input" maxlength="100" bind:value={draft.name} required />
			</div>
			<div>
				<label for="d-base">{t('dialing.baseMode')}</label>
				<select id="d-base" class="input" bind:value={draft.base_mode}>
					<option value="allow">{t('dialing.base.allow')}</option>
					<option value="deny">{t('dialing.base.deny')}</option>
				</select>
			</div>
		</div>
		<p class="hint">{t('dialing.baseHint')}</p>
		<div>
			<label for="d-desc">{t('dialing.description')}</label>
			<input id="d-desc" class="input" maxlength="500" bind:value={draft.description} />
		</div>
		<label class="flex items-center gap-2"
			><input type="checkbox" bind:checked={draft.sip_addresses} /> {t('dialing.sip')}</label
		>

		<fieldset class="space-y-2 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
			<legend class="px-1 text-sm font-medium">{t('dialing.rules')}</legend>
			<p class="hint">{t('dialing.rulesHint')}</p>
			{#if draft.rules.length}
				<div class="scroll-x max-h-72 overflow-y-auto">
					<table class="w-full text-left text-sm">
						<thead class="text-slate-500 dark:text-slate-400">
							<tr>
								<th class="py-1">{t('dialing.value')}</th>
								<th class="py-1">{t('dialing.action')}</th>
								<th class="col-opt py-1">{t('dialing.label')}</th>
								<th></th>
							</tr>
						</thead>
						<tbody>
							{#each draft.rules as r (`${r.kind}:${r.value}`)}
								<tr class="border-t border-slate-200 dark:border-slate-700">
									<td class="py-1">
										{#if r.kind === 'country'}<CountryFlag country={r.value} />
										{:else}<span class="text-xs text-slate-500 dark:text-slate-400"
												>{t(`dialing.kind.${r.kind}`)}</span
											>
											<span class="font-mono">{r.value}{r.kind === 'prefix' ? '*' : ''}</span>{/if}
									</td>
									<td class="py-1">
										<select
											class="input py-0.5"
											bind:value={r.action}
											aria-label={t('dialing.action')}
										>
											<option value="allow">{t('dialing.action.allow')}</option>
											<option value="deny">{t('dialing.action.deny')}</option>
										</select>
									</td>
									<td class="col-opt py-1 text-slate-600 dark:text-slate-300">{r.label}</td>
									<td class="py-1 text-right">
										<button
											type="button"
											class="btn btn-sm"
											aria-label={t('dialing.removeRule', { value: r.value })}
											onclick={() => removeRule(r)}>✕</button
										>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{:else}
				<p class="text-sm text-slate-500 dark:text-slate-400">{t('dialing.noRules')}</p>
			{/if}
			<form class="grid grid-cols-1 gap-2 @lg:grid-cols-2" onsubmit={addRule}>
				<div>
					<label for="d-kind">{t('dialing.kind')}</label>
					<select
						id="d-kind"
						class="input"
						bind:value={kind}
						onchange={() => {
							value = '';
							valueError = '';
						}}
					>
						<option value="country">{t('dialing.kind.country')}</option>
						<option value="prefix">{t('dialing.kind.prefix')}</option>
						<option value="number">{t('dialing.kind.number')}</option>
					</select>
				</div>
				<div>
					<label for="d-value">{t('dialing.value')}</label>
					{#if kind === 'country'}
						<div class="flex items-center gap-2">
							<select id="d-value" class="input" bind:value required>
								<option value="">{t('dialing.selectCountry')}</option>
								{#each countries as c (c.code)}<option value={c.code}>{c.name} ({c.code})</option
									>{/each}
							</select>
							{#if value}<CountryFlag country={value} name={false} />{/if}
						</div>
					{:else}
						<input
							id="d-value"
							class="input font-mono"
							placeholder={kind === 'prefix' ? '+49900*' : '+4930123456'}
							bind:value
							required
						/>
					{/if}
					{#if valueError}<p class="text-xs text-red-600 dark:text-red-400">{valueError}</p>{/if}
				</div>
				<div>
					<label for="d-action">{t('dialing.action')}</label>
					<select id="d-action" class="input" bind:value={action}>
						<option value="deny">{t('dialing.action.deny')}</option>
						<option value="allow">{t('dialing.action.allow')}</option>
					</select>
				</div>
				<div>
					<label for="d-label">{t('dialing.label')}</label>
					<input id="d-label" class="input" maxlength="200" bind:value={label} />
				</div>
				<div class="flex flex-wrap justify-between gap-2 @lg:col-span-2">
					<button
						type="button"
						class="btn btn-sm"
						title={t('dialing.premiumHint')}
						disabled={!missingPremium(draft.rules, premium).length}
						onclick={addPremium}>{t('dialing.addPremium')}</button
					>
					<button class="btn btn-sm">{t('dialing.addRule')}</button>
				</div>
			</form>
			<p class="hint">{t('dialing.premiumHint')}</p>
		</fieldset>

		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (open = false)}>{t('common.cancel')}</button>
			<button type="button" class="btn btn-primary" disabled={!draft.name.trim()} onclick={save}
				>{t('common.save')}</button
			>
		</div>
	</div>
</Modal>
