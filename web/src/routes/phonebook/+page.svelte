<script lang="ts">
	import SkeletonRows from '#lib/components/SkeletonRows.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { onMount } from 'svelte';
	import { api, type Contact, type PhonebookSection } from '#lib/api.ts';
	import ContactImport from '#lib/components/ContactImport.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let contacts = $state<Contact[]>([]);
	let sections = $state<PhonebookSection[]>([]);
	/** '' = all, 'global' = without section, else a section id. */
	let shown = $state('');
	let sectionsOpen = $state(false);
	let importOpen = $state(false);
	let newSection = $state('');
	let error = $state('');
	let search = $state('');
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

	const filtered = $derived(
		contacts.filter(
			(c) =>
				(shown === '' || (shown === 'global' ? c.section_id === null : c.section_id === shown)) &&
				[c.name, c.company, c.phone_work, c.phone_mobile, c.phone_other].some((v) =>
					v.toLowerCase().includes(search.trim().toLowerCase())
				)
		)
	);

	const sectionName = (id: string | null) =>
		id ? (sections.find((s) => s.id === id)?.name ?? '') : t('contacts.global');

	async function load() {
		try {
			[contacts, sections] = await Promise.all([
				api.get<Contact[]>('/contacts'),
				api.get<PhonebookSection[]>('/phonebook-sections')
			]);
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	function open(c: Contact | null) {
		editing = c;
		form = c
			? { ...c, section_id: c.section_id ?? '' }
			: { ...empty(), section_id: shown === '' || shown === 'global' ? '' : shown };
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
			if (shown === sec.id) shown = '';
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
	<div class="flex flex-wrap items-center justify-between gap-2">
		<h1>{t('nav.phonebook')}</h1>
		{#if hasRole('operator')}
			<div class="space-x-1">
				<button class="btn" onclick={() => (sectionsOpen = true)}>{t('contacts.sections')}</button>
				<button class="btn" onclick={() => (importOpen = true)}>{t('contacts.import')}</button>
				<button class="btn btn-primary" onclick={() => open(null)}>{t('contacts.new')}</button>
			</div>
		{/if}
	</div>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('contacts.hint')}</p>
	<ErrorBox {error} />
	<div class="flex flex-wrap items-center gap-2">
		<input class="input max-w-sm" placeholder={t('common.search')} bind:value={search} />
		{#if sections.length > 0}
			<select class="input w-auto" bind:value={shown} aria-label={t('contacts.section')}>
				<option value="">{t('contacts.allSections')}</option>
				<option value="global">{t('contacts.global')}</option>
				{#each sections as sec (sec.id)}<option value={sec.id}>{sec.name}</option>{/each}
			</select>
		{/if}
	</div>
	<div class="card overflow-x-auto">
		{#if contacts.length === 0 && net.settled}
			<p class="text-sm text-slate-500">{t('contacts.none')}</p>
		{:else}
			<table class="table">
				<thead>
					<tr
						><th>{t('common.name')}</th><th>{t('contacts.work')}</th><th>{t('contacts.mobile')}</th
						><th>{t('contacts.other')}</th><th>{t('contacts.section')}</th><th></th></tr
					>
				</thead>
				<tbody>
					{#each filtered as c (c.id)}
						<tr>
							<td
								>{c.name}{#if c.company}<span class="text-sm text-slate-500">
										· {c.company}</span
									>{/if}</td
							>
							<td class="font-mono text-sm">{c.phone_work}</td>
							<td class="font-mono text-sm">{c.phone_mobile}</td>
							<td class="font-mono text-sm">{c.phone_other}</td>
							<td class="text-sm text-slate-500">{sectionName(c.section_id)}</td>
							<td class="space-x-1 text-right whitespace-nowrap">
								{#if hasRole('operator')}
									<button class="btn btn-sm" onclick={() => open(c)}>{t('common.edit')}</button>
									<button class="btn btn-sm btn-danger" onclick={() => remove(c)}
										>{t('common.delete')}</button
									>
								{/if}
							</td>
						</tr>
					{:else}
						{#if !net.settled}<SkeletonRows cols={6} />{/if}
					{/each}
				</tbody>
			</table>
		{/if}
	</div>
</div>

<Modal title={editing ? t('common.edit') : t('contacts.new')} bind:open={editOpen}>
	<form class="space-y-3" onsubmit={save}>
		<div class="grid gap-3 sm:grid-cols-2">
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
