<script lang="ts">
	import { keyNeedsAccount, keyNeedsNumber } from '#lib/provisioning.ts';
	import { tick } from 'svelte';
	import type {
		Contact,
		Extension,
		KeyType,
		LineKey,
		PhoneAccount,
		PhoneModel,
		PhonebookSection
	} from '#lib/api.ts';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import {
		contactNumbers,
		contactsForPhone,
		effectivePhoneKey,
		keyValidation,
		phoneKeyPages,
		unsupportedKey
	} from '#lib/phoneKeys.ts';
	import Modal from './Modal.svelte';
	import Tabs from './Tabs.svelte';
	import PhoneGraphic from './PhoneGraphic.svelte';

	let {
		model,
		accounts,
		extensions,
		contacts,
		sections,
		sectionIds,
		keys = $bindable(),
		disabled = false
	}: {
		model: PhoneModel;
		accounts: PhoneAccount[];
		extensions: Extension[];
		contacts: Contact[];
		sections: PhonebookSection[];
		sectionIds: string[];
		keys: LineKey[];
		disabled?: boolean;
	} = $props();
	let page = $state(0);
	let view = $state<'graphic' | 'list'>('graphic');
	let open = $state(false);
	let draft = $state<LineKey>({ key: 1, type: 'speed_dial', value: '', label: '', account: 1 });
	let source = $state<'book' | 'internal' | 'manual'>('book');
	let book = $state('');
	let search = $state('');
	let contactId = $state('');
	let extensionNumber = $state('');
	let error = $state('');
	let trigger: HTMLElement | undefined;
	const pages = $derived(phoneKeyPages(model));
	const selectedPage = $derived(Math.min(page, pages.length - 1));
	const cells = $derived(pages[selectedPage] ?? []);
	const invalid = $derived(keys.filter((k) => unsupportedKey(k, model)));
	const visibleBooks = $derived(sections.filter((s) => sectionIds.includes(s.id)));
	const matches = $derived(contactsForPhone(contacts, sectionIds, book, search));
	const selectedContact = $derived(matches.find((c) => c.id === contactId));
	const numbers = $derived(selectedContact ? contactNumbers(selectedContact) : []);
	const types = $derived<KeyType[]>(
		(
			model.key_capabilities?.line_functions ?? [
				'line',
				'blf',
				...(model.speed_dial_keys ? ['speed_dial' as const] : []),
				'none'
			]
		).filter((kind) => kind !== 'line' || draft.key <= (model.line_key_limit ?? model.accounts))
	);
	const targetNeeded = $derived(keyNeedsNumber(draft.type) || draft.type === 'xml_browser');
	const sources = $derived(sourceTabs());
	function sourceTabs(): { id: 'book' | 'internal' | 'manual'; label: string }[] {
		return [
			...(draft.type === 'speed_dial' ? [{ id: 'book' as const, label: t('nav.phonebook') }] : []),
			{ id: 'internal' as const, label: t('phonekeys.internal') },
			{ id: 'manual' as const, label: t('phonekeys.manual') }
		];
	}
	function effective(n: number) {
		return effectivePhoneKey(n, keys, accounts);
	}
	function title(n: number) {
		const k = effective(n);
		return k.label || t(`keytype.${k.type}`);
	}
	function detail(n: number) {
		const k = effective(n);
		return k.type === 'line'
			? `${t('phones.account')} ${k.account}`
			: k.type === 'none'
				? t('phonekeys.configure')
				: `${t(`keytype.${k.type}`)} · ${k.value}`;
	}
	function keyName(n: number) {
		return `${t('phonekeys.key', { n })}: ${title(n)}`;
	}
	function pageName(p: number) {
		return t('phonekeys.page', { n: p + 1 });
	}
	function remove(n: number) {
		keys = keys.filter((k) => k.key !== n);
	}
	function pickContact(id: string) {
		contactId = id;
		const c = matches.find((c) => c.id === id);
		draft.value = c ? (contactNumbers(c)[0]?.value ?? '') : '';
		if (c) draft.label = Array.from(c.name).slice(0, 32).join('');
	}
	function pickExtension(number: string) {
		extensionNumber = number;
		draft.value = number ? (['blf', 'pickup'].includes(draft.type) ? number : `**${number}`) : '';
		const ext = extensions.find((e) => e.number === number);
		if (ext) draft.label = Array.from(ext.display_name).slice(0, 32).join('');
	}
	function changeSource(next: typeof source) {
		source = next;
		error = '';
		if (next === 'internal') pickExtension(extensionNumber);
		else if (next === 'book') pickContact('');
	}
	function changeType() {
		error = '';
		if (draft.type === 'blf') {
			source = 'internal';
			pickExtension(extensionNumber);
		} else if (!['speed_dial', 'intercom', 'pickup'].includes(draft.type)) source = 'manual';
		else if (source === 'internal') pickExtension(extensionNumber);
	}
	function edit(n: number, event: MouseEvent) {
		if (disabled) return;
		trigger = event.currentTarget as HTMLElement;
		const key = effective(n);
		draft = { ...key };
		if (key.type === 'none') {
			draft.type = model.speed_dial_keys ? 'speed_dial' : 'blf';
			draft.account = accounts[0]?.account_index ?? 1;
		}
		error = '';
		search = '';
		book = '';
		contactId = '';
		extensionNumber =
			extensions.find((e) => (key.type === 'blf' ? e.number : `**${e.number}`) === key.value)
				?.number ?? '';
		const contact = contactsForPhone(contacts, sectionIds).find((c) =>
			contactNumbers(c).some((n) => n.value === key.value)
		);
		if (extensionNumber) source = 'internal';
		else if (contact && draft.type === 'speed_dial') {
			source = 'book';
			contactId = contact.id;
		} else source = key.value ? 'manual' : draft.type === 'blf' ? 'internal' : 'book';
		open = true;
	}
	function apply() {
		error = '';
		if (
			targetNeeded &&
			source === 'book' &&
			(!selectedContact || !numbers.some((n) => n.value === draft.value))
		) {
			error = t('phonekeys.chooseContact');
			return;
		}
		const problem = keyValidation(draft, model, accounts);
		if (problem) {
			error = t(problem as MessageKey);
			return;
		}
		const key = {
			...draft,
			value: targetNeeded ? draft.value : '',
			label: draft.type === 'none' ? '' : draft.label
		};
		keys = [...keys.filter((k) => k.key !== key.key), key].sort((a, b) => a.key - b.key);
		open = false;
	}
	/** Keep the existing shared modal and contain focus for this editor. */
	function dialogFocus(node: HTMLElement) {
		const panel = node.closest<HTMLElement>('[role="dialog"]');
		void tick().then(() => node.querySelector<HTMLElement>('select')?.focus());
		function trap(event: KeyboardEvent) {
			if (event.key !== 'Tab' || !panel) return;
			const items = [
				...panel.querySelectorAll<HTMLElement>('button, input, select, [tabindex="0"]')
			].filter((el) => !el.hasAttribute('disabled') && el.getClientRects().length > 0);
			const first = items[0],
				last = items.at(-1);
			if (event.shiftKey && document.activeElement === first) {
				event.preventDefault();
				last?.focus();
			}
			if (!event.shiftKey && document.activeElement === last) {
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

<section class="card space-y-4" data-phone-keys>
	<div class="flex flex-wrap items-start justify-between gap-3">
		<div>
			<h2>{t('phonekeys.title')}</h2>
			<p class="hint">{t('phonekeys.hint')}</p>
		</div>
		{#if model.layout}<Tabs
				tabs={[
					{ id: 'graphic', label: t('phonekeys.graphic') },
					{ id: 'list', label: t('phonekeys.list') }
				]}
				bind:value={view}
				label={t('phonekeys.view')}
				variant="pills"
			/>{/if}
	</div>
	{#if accounts.length === 0}<p class="badge badge-warn">{t('phonekeys.noAccount')}</p>{/if}
	{#if model.layout && view === 'graphic'}
		{@const layout = model.layout}
		<div class="grid grid-cols-1 items-center gap-4 @3xl:grid-cols-[11rem_minmax(0,1fr)]">
			<div class="hidden @3xl:block">
				<PhoneGraphic {model} />
				<p class="mt-2 text-center text-sm font-medium">{model.name}</p>
			</div>
			<div class="min-w-0">
				<div
					class="phone-face"
					class:mono={!layout.color}
					class:touch={layout.touch}
					class:one-side={layout.left === 0 || layout.right === 0}
					class:right-only={layout.left === 0 && layout.right > 0}
				>
					<div class="face-brand"><span>Yealink</span><span>{model.name}</span></div>
					<div class="phone-screen">
						<div class="screen-head">
							<span>{t(layout.touch ? 'phonekeys.touchOverview' : 'phonekeys.layout')}</span><span
								>{layout.width} × {layout.height}</span
							>
						</div>
						<div class="screen-keys" style:--key-columns={layout.columns}>
							{#each cells as n, i (`${model.id}-${selectedPage}-${i}`)}
								{#if n}
									<button
										type="button"
										class="display-key"
										class:has-button={!layout.touch}
										class:key-left={layout.left > 0 && i % layout.columns === 0}
										class:key-right={layout.right > 0 &&
											(layout.columns === 1 || i % layout.columns === 1)}
										data-key={n}
										aria-label={keyName(n)}
										aria-haspopup="dialog"
										disabled={disabled || invalid.some((k) => k.key === n)}
										onclick={(e) => edit(n, e)}
									>
										<span class="key-number">{n}</span><span class="min-w-0 flex-1"
											><span class="key-title">{title(n)}</span><span class="key-detail"
												>{detail(n)}</span
											></span
										>
									</button>
								{:else}<button
										type="button"
										class="display-key page-key has-button key-right"
										aria-label={t('phonekeys.nextPage')}
										onclick={() => (page = (selectedPage + 1) % pages.length)}
										>{pageName(selectedPage)} →</button
									>{/if}
							{/each}
						</div>
					</div>
					<div class="face-bottom" aria-hidden="true">
						<span></span><span></span><span></span><span></span>
					</div>
				</div>
				<p class="hint mt-2 text-center">
					{t(layout.touch ? 'phonekeys.touchHint' : 'phonekeys.schematic')}
				</p>
			</div>
		</div>
	{:else}
		<div class="scroll-x">
			<table class="table">
				<thead
					><tr
						><th>#</th><th>{t('phones.keyType')}</th><th>{t('phones.keyValue')}</th><th
							>{t('phones.keyLabel')}</th
						><th></th></tr
					></thead
				><tbody>
					{#each Array.from({ length: model.line_keys }, (_, i) => i + 1) as n (n)}{@const key =
							effective(n)}
						<tr
							><td>{n}</td><td>{t(`keytype.${key.type}`)}</td><td class="font-mono text-xs"
								>{key.type === 'line' ? `${t('phones.account')} ${key.account}` : key.value}</td
							><td>{key.label}</td><td
								><button
									type="button"
									class="btn btn-sm"
									disabled={disabled || invalid.some((k) => k.key === n)}
									aria-label={keyName(n)}
									onclick={(e) => edit(n, e)}>{t('common.edit')}</button
								></td
							></tr
						>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
	{#if model.layout && view === 'graphic' && pages.length > 1}<div
			class="flex flex-wrap justify-center gap-2"
			aria-label={t('phonekeys.pages')}
		>
			{#each pages as _, p (p)}<button
					type="button"
					class="btn btn-sm"
					class:btn-primary={selectedPage === p}
					aria-pressed={selectedPage === p}
					onclick={() => (page = p)}>{pageName(p)}</button
				>{/each}
		</div>{/if}
	{#if invalid.length}
		<div
			class="space-y-2 rounded-lg border border-amber-300 bg-amber-50 p-3 text-sm dark:border-amber-800 dark:bg-amber-950/40"
		>
			<p>{t('phonekeys.legacy')}</p>
			{#each invalid as k (k.key)}<div class="flex flex-wrap items-center justify-between gap-2">
					<span
						>{t('phonekeys.key', { n: k.key })} · {k.label || t(`keytype.${k.type}`)} · {k.value}</span
					><button class="btn btn-sm" type="button" {disabled} onclick={() => remove(k.key)}
						>{t('common.delete')}</button
					>
				</div>{/each}
		</div>
	{/if}
</section>

<Modal title={t('phonekeys.editTitle', { n: draft.key })} bind:open>
	<form
		class="space-y-4"
		onsubmit={(e) => {
			e.preventDefault();
			apply();
		}}
		use:dialogFocus
	>
		<label class="block" for="key-function"
			>{t('phones.keyType')}<select
				id="key-function"
				aria-label={t('phones.keyType')}
				class="input"
				bind:value={draft.type}
				onchange={changeType}
				>{#each types as kind (kind)}<option value={kind}>{t(`keytype.${kind}`)}</option
					>{/each}</select
			></label
		>
		{#if targetNeeded}
			<Tabs
				tabs={sources}
				value={source}
				onchange={changeSource}
				label={t('phonekeys.source')}
				variant="pills"
			/>
			{#if source === 'book'}
				<label class="block" for="key-book"
					>{t('nav.phonebook')}<select
						id="key-book"
						aria-label={t('nav.phonebook')}
						class="input"
						bind:value={book}
						onchange={() => pickContact('')}
						><option value="">{t('phonekeys.assignedBooks')}</option><option value="global"
							>{t('contacts.global')}</option
						>{#each visibleBooks as section (section.id)}<option value={section.id}
								>{section.name}</option
							>{/each}</select
					></label
				>
				<label class="block" for="key-search"
					>{t('phonekeys.search')}<input
						id="key-search"
						type="search"
						class="input"
						bind:value={search}
					/></label
				>
				<div class="max-h-40 space-y-1 overflow-y-auto" aria-label={t('phonekeys.results')}>
					{#each matches as contact (contact.id)}<button
							type="button"
							class="contact-result"
							aria-pressed={contactId === contact.id}
							onclick={() => pickContact(contact.id)}
							><span class="min-w-0"
								><span class="block break-words font-medium">{contact.name}</span><span class="hint"
									>{contact.company}{contact.company ? ' · ' : ''}{contact.section_id
										? sections.find((s) => s.id === contact.section_id)?.name
										: t('contacts.global')}</span
								></span
							><span aria-hidden="true">{contactId === contact.id ? '✓' : '+'}</span></button
						>{:else}<p class="hint">{t('common.noMatches')}</p>{/each}
				</div>
				<label class="block" for="key-contact-number"
					>{t('phonekeys.number')}<select
						id="key-contact-number"
						aria-label={t('phonekeys.number')}
						class="input"
						bind:value={draft.value}
						disabled={!selectedContact}
						><option value="">{t('phonekeys.chooseNumber')}</option
						>{#each numbers as n (n.kind)}<option value={n.value}
								>{t(`phonekeys.${n.kind}`)} · {n.value}</option
							>{/each}</select
					></label
				>
			{:else if source === 'internal'}
				<label class="block" for="key-extension"
					>{t('phonekeys.internal')}<select
						id="key-extension"
						aria-label={t('phonekeys.internal')}
						class="input"
						value={extensionNumber}
						onchange={(e) => pickExtension(e.currentTarget.value)}
						><option value="">{t('phonekeys.chooseExtension')}</option
						>{#each extensions as extension (extension.id)}<option value={extension.number}
								>{extension.number} · {extension.display_name}</option
							>{/each}</select
					></label
				>
			{:else}
				<label class="block" for="key-number"
					>{t('phonekeys.number')}<input
						id="key-number"
						class="input font-mono"
						inputmode="tel"
						bind:value={draft.value}
						maxlength={draft.type === 'xml_browser' ? 400 : 32}
					/></label
				>
				<p class="hint">{t(draft.type === 'blf' ? 'phonekeys.blfHint' : 'phonekeys.numberHint')}</p>
			{/if}
		{/if}
		{#if draft.type !== 'none'}
			<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
				<label for="key-label"
					>{t('phones.keyLabel')}<input
						id="key-label"
						class="input"
						bind:value={draft.label}
						maxlength="32"
					/></label
				><label for="key-account" hidden={!keyNeedsAccount(draft.type)}
					>{t('phones.account')}<select
						id="key-account"
						aria-label={t('phones.account')}
						class="input"
						bind:value={draft.account}
						>{#if !accounts.some((a) => a.account_index === draft.account)}<option
								value={draft.account}
								>{t('phonekeys.unassignedAccount', { n: draft.account })}</option
							>{/if}{#each accounts as a (a.device_id)}<option value={a.account_index}
								>{a.account_index} · {a.extension_number} · {a.display_name}</option
							>{/each}</select
					></label
				>
			</div>
		{/if}
		{#if targetNeeded}<div
				class="flex flex-wrap justify-between gap-2 rounded-lg bg-slate-100 p-3 text-sm dark:bg-slate-800"
			>
				<span>{t(draft.type === 'blf' ? 'phonekeys.watches' : 'phonekeys.dials')}</span><strong
					class="font-mono break-all">{draft.value || '—'}</strong
				>
			</div>{/if}
		{#if error}<p class="text-sm text-red-700 dark:text-red-300" role="alert">{error}</p>{/if}
		<div class="flex flex-wrap items-center justify-end gap-2">
			<button
				type="button"
				class="btn btn-sm mr-auto"
				onclick={() => {
					remove(draft.key);
					open = false;
				}}>{t('phones.keyReset')}</button
			><button type="button" class="btn" onclick={() => (open = false)}>{t('common.cancel')}</button
			><button type="submit" class="btn btn-primary">{t('phonekeys.apply')}</button>
		</div>
	</form>
</Modal>

<style>
	.phone-face {
		border: 1px solid #526371;
		border-radius: 1.25rem;
		background: #273540;
		padding: 1rem 2.8rem 0.7rem;
		color: #e2e8f0;
		box-shadow: 0 8px 22px #0002;
	}
	.face-brand {
		display: flex;
		justify-content: space-between;
		gap: 0.5rem;
		font-size: 0.75rem;
		margin-bottom: 0.7rem;
	}
	.face-brand span:last-child {
		font-size: 0.6875rem;
	}
	.phone-screen {
		border: 3px solid #111e28;
		border-radius: 0.4rem;
		padding: 0.65rem;
		background: #edf6f4;
		color: #173e36;
	}
	.mono .phone-screen {
		background: #dce7dd;
		color: #293f32;
	}
	.screen-head {
		display: flex;
		justify-content: space-between;
		flex-wrap: wrap;
		gap: 0.4rem;
		font-size: 0.6875rem;
		margin-bottom: 0.7rem;
	}
	.screen-keys {
		display: grid;
		grid-template-columns: repeat(var(--key-columns), minmax(0, 1fr));
		gap: 0.5rem 1rem;
	}
	.touch .screen-keys {
		max-height: 25rem;
		overflow-y: auto;
		gap: 0.5rem;
	}
	.display-key {
		position: relative;
		display: flex;
		align-items: start;
		gap: 0.35rem;
		min-width: 0;
		min-height: 3rem;
		padding: 0.4rem;
		border: 1px solid #a6c5bb;
		border-radius: 0.35rem;
		background: #d3e7df;
		text-align: left;
		color: inherit;
	}
	.display-key:enabled:hover {
		background: #b7dccc;
	}
	.display-key:focus-visible {
		outline: 3px solid #0d9488;
		outline-offset: 2px;
	}
	.display-key:disabled {
		opacity: 0.7;
	}
	.key-title {
		display: block;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 0.75rem;
		font-weight: 500;
	}
	.key-detail {
		display: block;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 0.6875rem;
		opacity: 0.8;
	}
	.key-number {
		font-size: 0.6875rem;
		opacity: 0.7;
	}
	.has-button::before {
		content: '';
		position: absolute;
		top: calc(50% - 7px);
		width: 22px;
		height: 14px;
		border: 1px solid #79909f;
		border-radius: 20px;
		background: linear-gradient(#647f90, #3e5364);
		box-shadow: 0 2px 2px #0004;
	}
	.key-left::before {
		left: -3.1rem;
	}
	.key-right::before {
		right: -3.1rem;
	}
	.page-key {
		justify-content: center;
		align-items: center;
		background: transparent;
		border-style: dashed;
		font-size: 0.75rem;
	}
	.face-bottom {
		display: flex;
		justify-content: center;
		gap: 0.7rem;
		margin-top: 0.7rem;
	}
	.face-bottom span {
		height: 8px;
		width: 27px;
		background: #556c7c;
		border: 1px solid #7a8e9c;
		border-radius: 5px;
	}
	.one-side {
		padding-left: 1rem;
	}
	.one-side:not(.right-only) {
		padding-left: 2.8rem;
		padding-right: 1rem;
	}
	.touch {
		padding: 1rem;
	}
	.contact-result {
		display: flex;
		justify-content: space-between;
		gap: 0.5rem;
		width: 100%;
		padding: 0.5rem 0.65rem;
		border: 1px solid var(--color-slate-200);
		border-radius: 0.5rem;
		text-align: left;
	}
	.contact-result[aria-pressed='true'] {
		background: var(--color-teal-50);
		border-color: var(--color-teal-600);
	}
	:global(.dark) .contact-result {
		border-color: var(--color-slate-700);
	}
	:global(.dark) .contact-result[aria-pressed='true'] {
		background: var(--color-teal-950);
		border-color: var(--color-teal-400);
	}
	@container (max-width: 30rem) {
		.phone-face {
			padding: 0.7rem 1.9rem 0.6rem;
		}
		.phone-screen {
			padding: 0.4rem;
		}
		.screen-keys {
			gap: 0.4rem;
		}
		.key-left::before {
			left: -2.1rem;
			width: 16px;
		}
		.key-right::before {
			right: -2.1rem;
			width: 16px;
		}
		.display-key {
			padding: 0.3rem;
			gap: 0.2rem;
		}
		.one-side {
			padding-left: 0.6rem;
		}
		.one-side:not(.right-only) {
			padding-left: 1.9rem;
			padding-right: 0.6rem;
		}
		.touch {
			padding: 0.6rem;
		}
		.touch .screen-keys {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}
</style>
