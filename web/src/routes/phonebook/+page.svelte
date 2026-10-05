<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type Contact } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let contacts = $state<Contact[]>([]);
	let error = $state('');
	let search = $state('');
	let editOpen = $state(false);
	let editing = $state<Contact | null>(null);
	const empty = () => ({
		name: '',
		company: '',
		phone_work: '',
		phone_mobile: '',
		phone_other: ''
	});
	let form = $state(empty());

	const filtered = $derived(
		contacts.filter((c) =>
			[c.name, c.company, c.phone_work, c.phone_mobile, c.phone_other].some((v) =>
				v.toLowerCase().includes(search.trim().toLowerCase())
			)
		)
	);

	async function load() {
		try {
			contacts = await api.get<Contact[]>('/contacts');
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	function open(c: Contact | null) {
		editing = c;
		form = c ? { ...c } : empty();
		editOpen = true;
	}

	async function save(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		try {
			if (editing) await api.put(`/contacts/${editing.id}`, form);
			else await api.post('/contacts', form);
			editOpen = false;
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
			<button class="btn btn-primary" onclick={() => open(null)}>{t('contacts.new')}</button>
		{/if}
	</div>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('contacts.hint')}</p>
	<ErrorBox {error} />
	<input class="input max-w-sm" placeholder={t('common.search')} bind:value={search} />
	<div class="card overflow-x-auto">
		{#if contacts.length === 0}
			<p class="text-sm text-slate-500">{t('contacts.none')}</p>
		{:else}
			<table class="table">
				<thead>
					<tr
						><th>{t('common.name')}</th><th>{t('contacts.work')}</th><th>{t('contacts.mobile')}</th
						><th>{t('contacts.other')}</th><th></th></tr
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
							<td class="space-x-1 text-right whitespace-nowrap">
								{#if hasRole('operator')}
									<button class="btn btn-sm" onclick={() => open(c)}>{t('common.edit')}</button>
									<button class="btn btn-sm btn-danger" onclick={() => remove(c)}
										>{t('common.delete')}</button
									>
								{/if}
							</td>
						</tr>
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
