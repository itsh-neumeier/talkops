<script lang="ts">
	import PageHeader from '#lib/components/PageHeader.svelte';
	import SkeletonRows from '#lib/components/SkeletonRows.svelte';
	import ColumnFilter from '#lib/components/ColumnFilter.svelte';
	import FilterOptions from '#lib/components/FilterOptions.svelte';
	import SortButton from '#lib/components/SortButton.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { onMount, tick, untrack } from 'svelte';
	import { page } from '$app/state';
	import { api, type Contact, type PhonebookSection } from '#lib/api.ts';
	import ContactImport from '#lib/components/ContactImport.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { i18n, t } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import { syncAddress } from '#lib/list/address.ts';
	import { ariaSort, nextSort } from '#lib/list/sort.ts';
	import {
		GLOBAL,
		PRESENCES,
		activeFilters,
		anyFilter as hasFilter,
		emptyFilters,
		knownSections,
		matches,
		pageQuery,
		parseFilters,
		sortContacts,
		type NumberKind,
		type PhonebookFilters,
		type Presence,
		type SortKey
	} from '#lib/phonebook/filters.ts';

	let contacts = $state<Contact[]>([]);
	let sections = $state<PhonebookSection[]>([]);
	let filters = $state<PhonebookFilters>(parseFilters(page.url.searchParams));
	let sectionsOpen = $state(false);
	let importOpen = $state(false);
	let newSection = $state('');
	let error = $state('');
	let editOpen = $state(false);
	let editing = $state<Contact | null>(null);
	const empty = () => ({
		name: '',
		company: '',
		phone_work: '',
		phone_mobile: '',
		phone_other: '',
		section_id: '' as string
	});
	let form = $state(empty());
	/** Contact from `?contact=<id>` (call list, live calls): shown and highlighted. */
	let highlighted = $state('');
	/** The `?contact=` value that was already shown (not again after a filter change). */
	let handled = '';
	let loaded = $state(false);

	const sectionName = (id: string | null) =>
		id ? (sections.find((s) => s.id === id)?.name ?? '') : t('contacts.global');

	const active = $derived(activeFilters(filters));
	const anyFilter = $derived(hasFilter(filters));
	const filtered = $derived(
		sortContacts(
			contacts.filter((c) => matches(c, filters, sectionName(c.section_id))),
			filters,
			sectionName,
			i18n.locale
		)
	);
	const sectionOptions = $derived([
		{ value: GLOBAL, label: t('contacts.global') },
		...sections.map((s) => ({ value: s.id, label: s.name }))
	]);
	const numberColumns: { kind: NumberKind; label: string; opt: string }[] = $derived([
		{ kind: 'work', label: t('contacts.work'), opt: '' },
		{ kind: 'mobile', label: t('contacts.mobile'), opt: '' },
		{ kind: 'other', label: t('contacts.other'), opt: 'col-opt-lg' }
	]);
	const cols = $derived(sections.length > 0 ? 6 : 5);

	// The address follows the filters (a reload or a link shows the same list).
	$effect(() => {
		const query = pageQuery(filters, highlighted || (page.url.searchParams.get('contact') ?? ''));
		untrack(() => syncAddress('/phonebook', query));
	});

	async function load() {
		try {
			[contacts, sections] = await Promise.all([
				api.get<Contact[]>('/contacts'),
				api.get<PhonebookSection[]>('/phonebook-sections')
			]);
			const known = knownSections(
				filters,
				sections.map((s) => s.id)
			);
			if (known.length !== filters.section.length) filters.section = known;
			loaded = true;
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	$effect(() => {
		const id = page.url.searchParams.get('contact') ?? '';
		if (!loaded || !id || id === handled) return;
		// Runs once for each linked entry, not again after a filter change or a reload.
		untrack(() => {
			handled = id;
			const target = contacts.find((c) => c.id === id);
			if (!target) return;
			highlighted = id;
			// Show the phone book of the entry, so that it is visible.
			if (!matches(target, filters, sectionName(target.section_id))) {
				filters = {
					...emptyFilters(),
					sort: filters.sort,
					order: filters.order,
					section: sections.length > 0 ? [target.section_id ?? GLOBAL] : []
				};
			}
			const smooth = !matchMedia('(prefers-reduced-motion: reduce)').matches;
			tick().then(() =>
				document
					.getElementById(`contact-${id}`)
					?.scrollIntoView({ block: 'center', behavior: smooth ? 'smooth' : 'auto' })
			);
		});
	});

	function sortBy(key: SortKey) {
		({ sort: filters.sort, order: filters.order } = nextSort(filters, key, 'asc'));
	}

	function resetAll() {
		filters = { ...emptyFilters(), sort: filters.sort, order: filters.order };
	}

	const presenceLabel = (p: Presence) =>
		t(
			p === 'with'
				? 'contacts.withNumber'
				: p === 'without'
					? 'contacts.withoutNumber'
					: 'common.all'
		);

	function open(c: Contact | null) {
		editing = c;
		const one = filters.section.length === 1 ? filters.section[0] : GLOBAL;
		form = c
			? { ...c, section_id: c.section_id ?? '' }
			: { ...empty(), section_id: one === GLOBAL ? '' : one };
		editOpen = true;
	}

	async function save(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		try {
			const body = { ...form, section_id: form.section_id || null };
			if (editing) await api.put(`/contacts/${editing.id}`, body);
			else await api.post('/contacts', body);
			editOpen = false;
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function addSection(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		try {
			await api.post('/phonebook-sections', { name: newSection });
			newSection = '';
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function renameSection(sec: PhonebookSection) {
		const name = prompt(t('common.name'), sec.name);
		if (!name || name === sec.name) return;
		try {
			await api.put(`/phonebook-sections/${sec.id}`, { name });
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function removeSection(sec: PhonebookSection) {
		if (!confirm(t('contacts.deleteSectionConfirm', { name: sec.name }))) return;
		try {
			await api.del(`/phonebook-sections/${sec.id}`);
			filters.section = filters.section.filter((s) => s !== sec.id);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove(c: Contact) {
		if (!confirm(t('common.confirmDelete', { name: c.name }))) return;
		try {
			await api.del(`/contacts/${c.id}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<div class="space-y-4">
	<PageHeader title={t('nav.phonebook')} intro={t('contacts.hint')}>
		{#snippet actions()}
			{#if can('phonebook.edit')}
				<button class="btn" onclick={() => (sectionsOpen = true)}>{t('contacts.sections')}</button>
				<button class="btn" onclick={() => (importOpen = true)}>{t('contacts.import')}</button>
				<button class="btn btn-primary" onclick={() => open(null)}>{t('contacts.new')}</button>
			{/if}
		{/snippet}
	</PageHeader>
	<ErrorBox {error} />
	<div class="flex flex-wrap items-center gap-2">
		<input
			class="input max-w-sm"
			type="search"
			placeholder={t('contacts.searchPlaceholder')}
			aria-label={t('common.search')}
			bind:value={filters.search}
		/>
	</div>
	<div class="card scroll-x">
		{#if contacts.length === 0 && loaded && net.settled}
			<p class="text-sm text-slate-500">{t('contacts.none')}</p>
		{:else}
			<table class="table">
				<thead>
					<tr>
						<th
							aria-sort={ariaSort(
								filters.sort === 'name' || filters.sort === 'company',
								filters.order
							)}
						>
							<span class="inline-flex flex-wrap items-center gap-x-1 whitespace-nowrap">
								<SortButton
									label={t('common.name')}
									active={filters.sort === 'name'}
									order={filters.order}
									onclick={() => sortBy('name')}
								/>
								<span class="text-slate-400 dark:text-slate-500" aria-hidden="true">·</span>
								<SortButton
									label={t('contacts.company')}
									active={filters.sort === 'company'}
									order={filters.order}
									onclick={() => sortBy('company')}
								/>
							</span>
						</th>
						{#each numberColumns as col (col.kind)}
							<th class={col.opt}>
								<span class="inline-flex items-center gap-1 whitespace-nowrap">
									{col.label}
									<ColumnFilter
										label={col.label}
										active={active[col.kind]}
										onreset={() => (filters[col.kind] = '')}
									>
										<FilterOptions
											single
											options={PRESENCES.map((p) => ({ value: p, label: presenceLabel(p) }))}
											selected={[filters[col.kind]]}
											onchange={([v]) => (filters[col.kind] = v as Presence)}
										/>
									</ColumnFilter>
								</span>
							</th>
						{/each}
						{#if sections.length > 0}
							<th aria-sort={ariaSort(filters.sort === 'section', filters.order)}>
								<span class="inline-flex items-center gap-1 whitespace-nowrap">
									<SortButton
										label={t('contacts.section')}
										active={filters.sort === 'section'}
										order={filters.order}
										onclick={() => sortBy('section')}
									/>
									<ColumnFilter
										label={t('contacts.section')}
										active={active.section}
										onreset={() => (filters.section = [])}
									>
										<FilterOptions
											options={sectionOptions}
											selected={filters.section}
											onchange={(v) => (filters.section = v)}
										/>
									</ColumnFilter>
								</span>
							</th>
						{/if}
						<th></th>
					</tr>
				</thead>
				<tbody>
					{#each filtered as c (c.id)}
						<tr
							id="contact-{c.id}"
							class={highlighted === c.id
								? 'bg-teal-50 outline-2 -outline-offset-2 outline-teal-500 dark:bg-teal-950/60'
								: ''}
							aria-current={highlighted === c.id ? 'true' : undefined}
						>
							<td
								>{c.name}{#if c.company}<span class="text-sm text-slate-500">
										· {c.company}</span
									>{/if}</td
							>
							<td class="font-mono text-sm">{c.phone_work}</td>
							<td class="font-mono text-sm">{c.phone_mobile}</td>
							<td class="col-opt-lg font-mono text-sm">{c.phone_other}</td>
							{#if sections.length > 0}
								<td class="text-sm text-slate-500">{sectionName(c.section_id)}</td>
							{/if}
							<td class="space-x-1 text-right whitespace-nowrap">
								{#if can('phonebook.edit')}
									<button class="btn btn-sm" onclick={() => open(c)}>{t('common.edit')}</button>
									<button class="btn btn-sm btn-danger" onclick={() => remove(c)}
										>{t('common.delete')}</button
									>
								{/if}
							</td>
						</tr>
					{:else}
						{#if !loaded || !net.settled}<SkeletonRows {cols} />
						{:else}
							<tr>
								<td
									colspan={cols}
									class="py-8 text-center text-sm text-slate-500 dark:text-slate-400"
								>
									{t('contacts.noMatches')}
									{#if anyFilter}
										<div class="mt-2">
											<button class="btn btn-sm" onclick={resetAll}>{t('filter.resetAll')}</button>
										</div>
									{/if}
								</td>
							</tr>
						{/if}
					{/each}
				</tbody>
			</table>
			{#if loaded && contacts.length > 0}
				<div
					class="flex flex-wrap items-center justify-between gap-2 pt-3 text-sm text-slate-500 dark:text-slate-400"
				>
					<span>{t('contacts.shown', { shown: filtered.length, total: contacts.length })}</span>
					{#if anyFilter}
						<button class="btn btn-sm" onclick={resetAll}>{t('filter.resetAll')}</button>
					{/if}
				</div>
			{/if}
		{/if}
	</div>
</div>

<Modal title={editing ? t('common.edit') : t('contacts.new')} bind:open={editOpen}>
	<form class="space-y-3" onsubmit={save}>
		<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
			<div>
				<label for="c-name">{t('common.name')}</label>
				<input id="c-name" class="input" bind:value={form.name} required />
			</div>
			<div>
				<label for="c-company">{t('contacts.company')} ({t('common.optional')})</label>
				<input id="c-company" class="input" bind:value={form.company} />
			</div>
			<div>
				<label for="c-work">{t('contacts.work')}</label>
				<input id="c-work" class="input font-mono" bind:value={form.phone_work} inputmode="tel" />
			</div>
			<div>
				<label for="c-mobile">{t('contacts.mobile')}</label>
				<input
					id="c-mobile"
					class="input font-mono"
					bind:value={form.phone_mobile}
					inputmode="tel"
				/>
			</div>
			<div>
				<label for="c-other">{t('contacts.other')}</label>
				<input id="c-other" class="input font-mono" bind:value={form.phone_other} inputmode="tel" />
			</div>
			<div>
				<label for="c-section">{t('contacts.section')}</label>
				<select id="c-section" class="input" bind:value={form.section_id}>
					<option value="">{t('contacts.global')}</option>
					{#each sections as sec (sec.id)}<option value={sec.id}>{sec.name}</option>{/each}
				</select>
			</div>
		</div>
		<p class="hint">{t('contacts.numberHint')}</p>
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (editOpen = false)}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary">{t('common.save')}</button>
		</div>
	</form>
</Modal>

<Modal title={t('contacts.sections')} bind:open={sectionsOpen}>
	<div class="space-y-3">
		<p class="hint">{t('contacts.sectionsHint')}</p>
		{#if sections.length > 0}
			<ul class="divide-y divide-slate-100 dark:divide-slate-800">
				{#each sections as sec (sec.id)}
					<li class="flex items-center justify-between gap-2 py-2">
						<span>{sec.name}</span>
						<span class="space-x-1">
							<button class="btn btn-sm" onclick={() => renameSection(sec)}
								>{t('common.edit')}</button
							>
							<button class="btn btn-sm btn-danger" onclick={() => removeSection(sec)}
								>{t('common.delete')}</button
							>
						</span>
					</li>
				{/each}
			</ul>
		{/if}
		<form class="flex gap-2" onsubmit={addSection}>
			<input
				class="input"
				bind:value={newSection}
				maxlength="32"
				required
				placeholder={t('contacts.sectionPlaceholder')}
				aria-label={t('common.name')}
			/>
			<button class="btn">{t('common.add')}</button>
		</form>
	</div>
</Modal>

<ContactImport bind:open={importOpen} {sections} onimported={load} />
