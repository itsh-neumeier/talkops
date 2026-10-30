<script lang="ts">
	import DeviceLocaleFields from './DeviceLocaleFields.svelte';
	import { onMount, tick } from 'svelte';
	import {
		api,
		type PhoneModel,
		type PhoneAccount,
		type PhoneProvisioning,
		type ProvisioningKey,
		type ProvisioningTemplate,
		type Contact,
		type PhonebookSection,
		type KeyType
	} from '#lib/api.ts';
	import Modal from '#lib/components/Modal.svelte';
	import Tabs from '#lib/components/Tabs.svelte';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { keyNeedsAccount, keyNeedsNumber, putProvisioningKey } from '#lib/provisioning.ts';
	import { contactsForPhone, contactNumbers } from '#lib/phoneKeys.ts';
	let {
		model,
		accounts,
		contacts,
		sections,
		sectionIds,
		values = $bindable({}),
		disabled = false
	}: {
		model: PhoneModel;
		accounts: PhoneAccount[];
		contacts: Contact[];
		sections: PhonebookSection[];
		sectionIds: string[];
		values: PhoneProvisioning;
		disabled?: boolean;
	} = $props();
	let templates = $state<ProvisioningTemplate[]>([]);
	let group = $state('programmable');
	let open = $state(false);
	let draft = $state<ProvisioningKey>({
		group: 'programmable',
		key: 1,
		type: 'none',
		label: '',
		value: '',
		account: 1,
		history_type: 0,
		phonebook: 0
	});
	let inherited = $state(false);
	let contactId = $state('');
	let error = $state('');
	const caps = $derived(model.key_capabilities);
	const groups = $derived([
		...(caps?.programmable.length ? [{ id: 'programmable', label: t('prov.programmable') }] : []),
		...Array.from({ length: caps?.expansion_modules ?? 0 }, (_, i) => ({
			id: `expansion_${i + 1}`,
			label: t('prov.expansion', { n: i + 1 })
		})),
		...(Object.keys(caps?.softkey_states ?? {}).length
			? [{ id: 'softkeys', label: t('prov.callStates') }]
			: [])
	]);
	const activeGroup = $derived(groups.some((g) => g.id === group) ? group : (groups[0]?.id ?? ''));
	const slots = $derived(
		activeGroup === 'programmable'
			? (caps?.programmable ?? [])
			: Array.from({ length: caps?.expansion_keys ?? 0 }, (_, i) => i + 1)
	);
	const functions = $derived(
		draft.group === 'programmable'
			? (caps?.programmable_functions ?? [])
			: (caps?.line_functions ?? [])
	);
	const books = $derived(sections.filter((s) => sectionIds.includes(s.id)));
	const availableContacts = $derived(contactsForPhone(contacts, sectionIds));
	const picked = $derived(availableContacts.find((c) => c.id === contactId));
	const orphans = $derived(
		model.provisioning_capabilities?.native
			? []
			: (values.keys ?? []).filter((k) =>
					k.group === 'programmable'
						? !caps?.programmable.includes(k.key)
						: !groups.some((g) => g.id === k.group)
				)
	);
	onMount(async () => {
		if (!disabled)
			templates = await api.get<ProvisioningTemplate[]>('/provisioning/templates').catch(() => []);
	});
	function keyName(n: number) {
		return activeGroup === 'programmable'
			? t(`prov.key.${n}` as MessageKey)
			: t('phonekeys.key', { n });
	}
	let trigger: HTMLElement | undefined;
	function edit(n: number, event: MouseEvent) {
		trigger = event.currentTarget as HTMLElement;
		const previous = (values.keys ?? []).find((k) => k.group === activeGroup && k.key === n);
		inherited = !previous;
		draft = previous
			? { ...previous }
			: {
					group: activeGroup,
					key: n,
					type: 'none',
					label: '',
					value: '',
					account: accounts[0]?.account_index ?? 1,
					history_type: 0,
					phonebook: 0
				};
		contactId = '';
		error = '';
		open = true;
	}
	function apply() {
		if (
			!inherited &&
			keyNeedsNumber(draft.type) &&
			(!/^[0-9*#+]+$/.test(draft.value) || draft.value.length > 32)
		) {
			error = t('phonekeys.invalidNumber');
			return;
		}
		if (!inherited && draft.type === 'xml_browser' && !/^https?:\/\//.test(draft.value)) {
			error = t('prov.invalidUrl');
			return;
		}
		if (
			!inherited &&
			keyNeedsAccount(draft.type) &&
			!(draft.account === 0 && ['speed_dial', 'intercom'].includes(draft.type)) &&
			!accounts.some((a) => a.account_index === draft.account)
		) {
			error = t('phonekeys.missingAccount');
			return;
		}
		values = putProvisioningKey(
			values,
			inherited
				? null
				: {
						...draft,
						value: keyNeedsNumber(draft.type) || draft.type === 'xml_browser' ? draft.value : '',
						account: keyNeedsAccount(draft.type) ? draft.account : 0
					},
			draft.group,
			draft.key
		);
		open = false;
	}
	function chooseContact(id: string) {
		contactId = id;
		if (picked) {
			draft.value = contactNumbers(picked)[0]?.value ?? '';
			draft.label = Array.from(picked.name).slice(0, 32).join('');
		}
	}
	function setLayout(state: string, slot: number, action: string) {
		const current = values.softkeys?.[state] ?? ['Empty', 'Empty', 'Empty', 'Empty'];
		const next = [...current];
		next[slot] = action;
		values = { ...values, softkeys: { ...values.softkeys, [state]: next } };
	}
	function resetLayout(state: string) {
		const softkeys = { ...values.softkeys };
		delete softkeys[state];
		values = { ...values, softkeys };
	}
	function dialogFocus(node: HTMLElement) {
		const panel = node.closest<HTMLElement>('[role="dialog"]');
		void tick().then(() => node.querySelector<HTMLElement>('select')?.focus());
		function trap(event: KeyboardEvent) {
			if (event.key !== 'Tab' || !panel) return;
			const items = [...panel.querySelectorAll<HTMLElement>('button,input,select')].filter(
				(el) => !el.hasAttribute('disabled') && el.getClientRects().length > 0
			);
			const first = items[0],
				last = items.at(-1);
			if (event.shiftKey && document.activeElement === first) {
				event.preventDefault();
				last?.focus();
			} else if (!event.shiftKey && document.activeElement === last) {
				event.preventDefault();
				first?.focus();
			}
		}
		panel?.addEventListener('keydown', trap);
		return {
			destroy() {
				panel?.removeEventListener('keydown', trap);
				trigger?.focus();
			}
		};
	}
</script>

<section class="card space-y-4">
	<DeviceLocaleFields
		prefix="device-locale"
		bind:values
		{disabled}
		ciscoLocale={model.vendor === 'cisco' && model.id.endsWith('-enterprise')}
	/>
	<h2>{t('prov.deviceTitle')}</h2>
	<p class="hint">{t('prov.deviceHint')}</p>
	<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
		<label for="device-endpoint"
			>{t('prov.endpoint')}<input
				id="device-endpoint"
				class="input font-mono"
				placeholder="http://pbx.lan:8080/provisioning"
				bind:value={values.endpoint}
				{disabled}
			/></label
		>
		<label for="device-sip-host"
			>{t('prov.sipHost')}<input
				id="device-sip-host"
				class="input"
				bind:value={values.sip_host}
				{disabled}
			/></label
		>
		{#if caps?.firmware_profiles.length}<label for="device-key-profile"
				>{t('prov.firmwareProfile')}<select
					id="device-key-profile"
					class="input"
					bind:value={values.firmware_profile}
					{disabled}
					><option value="">{t('prov.currentProfile')}</option
					>{#each caps.firmware_profiles as profile}<option value={profile}>{profile}</option
						>{/each}</select
				></label
			>{/if}
		<label for="device-template"
			>{t('prov.template')}<select
				id="device-template"
				class="input"
				value={values.template_id ?? ''}
				onchange={(e) => (values = { ...values, template_id: e.currentTarget.value || null })}
				{disabled}
				><option value="">{t('prov.builtin')}</option
				>{#each templates.filter((p) => p.model === model.id && p.active_revision !== null) as template}<option
						value={template.id}>{template.name} · r{template.active_revision}</option
					>{/each}</select
			></label
		>
	</div>
	<p class="hint">{t('prov.untested')}</p>
	{#if groups.length}
		<Tabs
			tabs={groups}
			value={activeGroup}
			onchange={(next) => (group = next)}
			variant="pills"
			label={t('prov.groups')}
		/>
		{#if activeGroup === 'softkeys'}
			<p class="hint">{t('prov.layoutHint')}</p>
			{#each Object.entries(caps?.softkey_states ?? {}) as [state, actions]}
				<fieldset class="space-y-2">
					<legend class="font-medium">{t(`prov.state.${state}` as MessageKey)}</legend>
					<div class="grid grid-cols-1 gap-2 @lg:grid-cols-2 @3xl:grid-cols-4">
						{#each [0, 1, 2, 3] as slot}<label for="softkey-{state}-{slot}"
								>{t('prov.position', { n: slot + 1 })}<select
									aria-label={t('prov.position', { n: slot + 1 })}
									id="softkey-{state}-{slot}"
									class="input"
									value={values.softkeys?.[state]?.[slot] ?? ''}
									onchange={(e) => setLayout(state, slot, e.currentTarget.value)}
									{disabled}
									><option value="" disabled>{t('prov.factory')}</option
									>{#each actions as action}<option value={action}
											>{t(`prov.softkey.${action}` as MessageKey)}</option
										>{/each}</select
								></label
							>{/each}
					</div>
					<button type="button" class="btn btn-sm" onclick={() => resetLayout(state)} {disabled}
						>{t('phones.keyReset')}</button
					>
				</fieldset>
			{/each}
		{:else}
			<div class="grid grid-cols-1 gap-2 @lg:grid-cols-2 @3xl:grid-cols-3">
				{#each slots as slot}
					{@const key = (values.keys ?? []).find((k) => k.group === activeGroup && k.key === slot)}
					<button
						class="btn flex flex-col items-start text-left"
						type="button"
						onclick={(event) => edit(slot, event)}
						{disabled}
					>
						<span>{keyName(slot)}</span><span class="hint"
							>{key
								? `${t(`keytype.${key.type}`)}${key.label ? ` · ${key.label}` : ''}`
								: t('prov.factory')}</span
						>
					</button>
				{/each}
			</div>
		{/if}
	{/if}
	{#each orphans as key}<div class="flex flex-wrap items-center gap-2">
			<span>{t('phonekeys.legacy')} · {key.group}/{key.key}: {key.label}</span><button
				type="button"
				class="btn btn-sm"
				{disabled}
				onclick={() => (values = putProvisioningKey(values, null, key.group, key.key))}
				>{t('common.delete')}</button
			>
		</div>{/each}
</section>

<Modal bind:open title={t('prov.editKey')}>
	<form
		use:dialogFocus
		class="space-y-3"
		onsubmit={(e) => {
			e.preventDefault();
			apply();
		}}
	>
		<p class="hint">{draft.group} · {draft.key}</p>
		<label for="provision-function"
			>{t('phones.keyType')}<select
				id="provision-function"
				aria-label={t('phones.keyType')}
				class="input"
				value={inherited ? 'inherit' : draft.type}
				onchange={(e) => {
					inherited = e.currentTarget.value === 'inherit';
					if (!inherited) draft.type = e.currentTarget.value as KeyType;
				}}
				><option value="inherit">{t('prov.factory')}</option>{#each functions as fn}<option
						value={fn}>{t(`keytype.${fn}`)}</option
					>{/each}</select
			></label
		>
		{#if !inherited}
			{#if keyNeedsNumber(draft.type) || draft.type === 'xml_browser'}
				{#if draft.type === 'speed_dial'}
					<label for="provision-contact"
						>{t('nav.phonebook')}<select
							id="provision-contact"
							aria-label={t('nav.phonebook')}
							class="input"
							onchange={(e) => chooseContact(e.currentTarget.value)}
							><option value="">{t('phonekeys.manual')}</option
							>{#each availableContacts as c}<option value={c.id}>{c.name}</option>{/each}</select
						></label
					>
					{#if picked}<label for="provision-contact-number"
							>{t('phonekeys.number')}<select
								id="provision-contact-number"
								aria-label={t('phonekeys.number')}
								class="input"
								bind:value={draft.value}
								>{#each contactNumbers(picked) as n}<option value={n.value}
										>{t(`phonekeys.${n.kind}`)} · {n.value}</option
									>{/each}</select
							></label
						>{/if}
				{/if}
				<label for="provision-value"
					>{t('phones.keyValue')}<input
						id="provision-value"
						class="input font-mono"
						bind:value={draft.value}
						maxlength={draft.type === 'xml_browser' ? 400 : 32}
					/></label
				>
			{/if}
			{#if draft.type === 'history'}<label for="provision-history"
					>{t('prov.historySource')}<select
						id="provision-history"
						class="input"
						bind:value={draft.history_type}
						><option value={0}>{t('prov.localHistory')}</option><option disabled
							>{t('prov.externalService')}</option
						></select
					></label
				>{/if}
			{#if draft.type === 'xml_phonebook'}<label for="provision-book"
					>{t('nav.phonebook')}<select
						id="provision-book"
						aria-label={t('nav.phonebook')}
						class="input"
						bind:value={draft.phonebook}
						><option value={0}>{t('phonekeys.internal')}</option><option value={1}
							>{t('contacts.global')}</option
						>{#each books as book, index}<option value={index + 2}>{book.name}</option
							>{/each}</select
					></label
				>{/if}
			<label for="provision-label"
				>{t('phones.keyLabel')}<input
					id="provision-label"
					class="input"
					bind:value={draft.label}
					maxlength="32"
				/></label
			>
			{#if keyNeedsAccount(draft.type)}<label for="provision-account"
					>{t('phones.account')}<select
						id="provision-account"
						aria-label={t('phones.account')}
						class="input"
						bind:value={draft.account}
						>{#if ['speed_dial', 'intercom'].includes(draft.type)}<option value={0}
								>{t('prov.autoAccount')}</option
							>{/if}{#each accounts as account}<option value={account.account_index}
								>{account.account_index} · {account.extension_number}</option
							>{/each}</select
					></label
				>{/if}
		{/if}
		{#if error}<p class="text-red-600" role="alert">{error}</p>{/if}
		<div class="flex flex-wrap justify-end gap-2">
			<button class="btn" type="button" onclick={() => (open = false)}>{t('common.cancel')}</button
			><button class="btn btn-primary" type="submit">{t('common.apply')}</button>
		</div>
	</form>
</Modal>
