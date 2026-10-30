<script lang="ts">
	import { onMount } from 'svelte';
	import { api, getText, type PhoneDetail, type PhoneListItem } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	/** Admin-stored provisioning files (ADR 0052): a file with the name of a
	 * generated file replaces it; another name adds a file. */
	let { phones }: { phones: PhoneListItem[] } = $props();

	type Transport = 'http' | 'tftp';
	type StoredFile = {
		id: string;
		transport: Transport;
		phone_id: string | null;
		path: string;
		size_bytes: number;
		updated_at: string;
	};
	type Delivered = { transport: Transport; path: string; artifact: string; name: string };
	type Draft = {
		transport: Transport;
		phone_id: string | null;
		path: string;
		content: string;
		/** Set for binary files: saved unchanged unless a new file is chosen. */
		base64: string | null;
		existing: boolean;
	};

	let files = $state<StoredFile[]>([]);
	let error = $state('');
	let phoneId = $state('');
	let delivered = $state<Delivered[]>([]);
	let draft = $state<Draft | null>(null);
	let upload = $state<FileList | null>(null);
	let saving = $state(false);
	let viewing = $state<{ title: string; content: string } | null>(null);
	let draftOpen = $state(false);
	let viewOpen = $state(false);

	const phoneName = (id: string | null) =>
		id ? (phones.find((p) => p.id === id)?.name ?? id) : t('provfiles.allPhones');

	async function load() {
		try {
			files = await api.get<StoredFile[]>('/provisioning-files');
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	/** Stored file that the phone gets at this path: its own first, then global. */
	function stored(transport: Transport, path: string, phone: string | null) {
		return (
			files.find((f) => f.transport === transport && f.path === path && f.phone_id === phone) ??
			files.find((f) => f.transport === transport && f.path === path && f.phone_id === null)
		);
	}

	async function selectPhone() {
		delivered = [];
		if (!phoneId) return;
		try {
			const detail = await api.get<PhoneDetail>(`/phones/${phoneId}`);
			const transport: Transport = detail.delivery === 'tftp' ? 'tftp' : 'http';
			delivered = (detail.provisioning_files ?? []).map((f) => ({
				transport,
				artifact: f.artifact,
				name: f.name,
				path:
					transport === 'tftp'
						? f.name
						: (new URL(f.url, location.href).pathname.split('/provisioning/')[1] ?? f.name)
			}));
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function generated(file: Delivered) {
		return getText(`/phones/${phoneId}/config?artifact=${encodeURIComponent(file.artifact)}`);
	}

	async function view(file: Delivered) {
		error = '';
		try {
			viewing = { title: file.path, content: await generated(file) };
			viewOpen = true;
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function editDelivered(file: Delivered) {
		error = '';
		const own = stored(file.transport, file.path, file.transport === 'tftp' ? phoneId : null);
		if (own) return editStored(own);
		try {
			draft = {
				transport: file.transport,
				phone_id: file.transport === 'tftp' ? phoneId : null,
				path: file.path,
				content: await generated(file),
				base64: null,
				existing: false
			};
			draftOpen = true;
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function editStored(file: StoredFile) {
		error = '';
		try {
			const view = await api.get<{ content: string | null; content_base64: string }>(
				`/provisioning-files/${file.id}`
			);
			draft = {
				transport: file.transport,
				phone_id: file.phone_id,
				path: file.path,
				content: view.content ?? '',
				base64: view.content === null ? view.content_base64 : null,
				existing: true
			};
			draftOpen = true;
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function newFile() {
		error = '';
		draft = {
			transport: 'tftp',
			phone_id: null,
			path: '',
			content: '',
			base64: null,
			existing: false
		};
		draftOpen = true;
	}

	function toBase64(buffer: ArrayBuffer) {
		let binary = '';
		const bytes = new Uint8Array(buffer);
		for (let i = 0; i < bytes.length; i += 0x8000)
			binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
		return btoa(binary);
	}

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!draft) return;
		error = '';
		saving = true;
		try {
			const chosen = upload?.[0];
			const body: Record<string, unknown> = {
				transport: draft.transport,
				phone_id: draft.transport === 'tftp' ? draft.phone_id : null,
				path: draft.path.trim()
			};
			if (chosen) body.content_base64 = toBase64(await chosen.arrayBuffer());
			else if (draft.base64 !== null) body.content_base64 = draft.base64;
			else body.content = draft.content;
			await api.put('/provisioning-files', body);
			draftOpen = false;
			upload = null;
			await load();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			saving = false;
		}
	}

	async function remove(file: StoredFile) {
		if (!confirm(t('common.confirmDelete', { name: file.path }))) return;
		try {
			await api.del(`/provisioning-files/${file.id}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<section class="card space-y-3">
	<h2>{t('provfiles.title')}</h2>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('provfiles.hint')}</p>
	<ErrorBox {error} />

	<div class="flex flex-wrap items-end gap-2">
		<div>
			<label for="provfiles-phone">{t('provfiles.phone')}</label>
			<select id="provfiles-phone" class="input" bind:value={phoneId} onchange={selectPhone}>
				<option value="">{t('provfiles.choosePhone')}</option>
				{#each phones as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
			</select>
		</div>
		<button class="btn" type="button" onclick={newFile}>{t('provfiles.new')}</button>
	</div>

	{#if phoneId && delivered.length > 0}
		<h3 class="font-medium">{t('provfiles.delivered')}</h3>
		<div class="scroll-x">
			<table class="table">
				<thead>
					<tr
						><th>{t('provfiles.path')}</th><th>{t('provfiles.transport')}</th><th
							>{t('provfiles.state')}</th
						><th></th></tr
					>
				</thead>
				<tbody>
					{#each delivered as file (file.transport + file.path)}
						{@const own = stored(
							file.transport,
							file.path,
							file.transport === 'tftp' ? phoneId : null
						)}
						<tr>
							<td class="font-mono text-xs">{file.path}</td>
							<td class="text-sm uppercase">{file.transport}</td>
							<td class="text-sm"
								>{#if own}<span class="badge badge-warn">{t('provfiles.replaced')}</span>{:else}{t(
										'provfiles.generated'
									)}{/if}</td
							>
							<td class="space-x-1 text-right whitespace-nowrap">
								<button class="btn btn-sm" onclick={() => view(file)}
									>{t('provfiles.viewGenerated')}</button
								>
								<button class="btn btn-sm" onclick={() => editDelivered(file)}
									>{t('common.edit')}</button
								>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}

	<h3 class="font-medium">{t('provfiles.stored')}</h3>
	{#if files.length === 0}
		<p class="text-sm text-slate-500">{t('provfiles.none')}</p>
	{:else}
		<div class="scroll-x">
			<table class="table">
				<thead>
					<tr
						><th>{t('provfiles.path')}</th><th>{t('provfiles.transport')}</th><th
							>{t('provfiles.scope')}</th
						><th>{t('phones.size')}</th><th>{t('provfiles.updated')}</th><th></th></tr
					>
				</thead>
				<tbody>
					{#each files as file (file.id)}
						<tr>
							<td class="font-mono text-xs">{file.path}</td>
							<td class="text-sm uppercase">{file.transport}</td>
							<td class="text-sm"
								>{file.transport === 'http'
									? t('provfiles.allPhones')
									: phoneName(file.phone_id)}</td
							>
							<td class="text-sm">{(file.size_bytes / 1024).toFixed(1)} KB</td>
							<td class="text-sm">{formatDateTime(file.updated_at)}</td>
							<td class="space-x-1 text-right whitespace-nowrap">
								<button class="btn btn-sm" onclick={() => editStored(file)}
									>{t('common.edit')}</button
								>
								<button class="btn btn-sm btn-danger" onclick={() => remove(file)}
									>{t('provfiles.remove')}</button
								>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</section>

<Modal title={viewing?.title ?? ''} bind:open={viewOpen}>
	{#if viewing}
		<pre class="max-h-[60vh] overflow-auto text-xs whitespace-pre-wrap">{viewing.content}</pre>
	{/if}
</Modal>

<Modal
	title={draft?.existing ? t('provfiles.editTitle') : t('provfiles.newTitle')}
	bind:open={draftOpen}
>
	{#if draft}
		<form class="space-y-3" onsubmit={save}>
			<div class="grid grid-cols-1 gap-2 @xl:grid-cols-3">
				<div>
					<label for="provfiles-transport">{t('provfiles.transport')}</label>
					<select
						id="provfiles-transport"
						class="input"
						bind:value={draft.transport}
						disabled={draft.existing}
					>
						<option value="tftp">TFTP (Cisco Enterprise)</option>
						<option value="http">HTTP</option>
					</select>
				</div>
				<div>
					<label for="provfiles-scope">{t('provfiles.scope')}</label>
					<select
						id="provfiles-scope"
						class="input"
						bind:value={draft.phone_id}
						disabled={draft.existing || draft.transport === 'http'}
					>
						<option value={null}>{t('provfiles.allPhones')}</option>
						{#each phones as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
					</select>
				</div>
				<div>
					<label for="provfiles-path">{t('provfiles.path')}</label>
					<input
						id="provfiles-path"
						class="input font-mono"
						bind:value={draft.path}
						readonly={draft.existing}
						required
						placeholder={draft.transport === 'tftp' ? 'g3-tones.xml' : 'custom/file.xml'}
					/>
				</div>
			</div>
			<p class="text-xs text-slate-500">
				{draft.transport === 'tftp' ? t('provfiles.tftpNames') : t('provfiles.httpNames')}
			</p>
			{#if draft.base64 === null}
				<textarea
					class="input h-[50vh] w-full font-mono text-xs"
					spellcheck="false"
					aria-label={t('provfiles.content')}
					bind:value={draft.content}></textarea>
			{:else}
				<p class="text-sm">{t('provfiles.binary')}</p>
			{/if}
			<div>
				<label for="provfiles-upload">{t('provfiles.upload')}</label>
				<input id="provfiles-upload" class="input" type="file" bind:files={upload} />
			</div>
			<p class="text-xs text-amber-700 dark:text-amber-400">{t('provfiles.warning')}</p>
			<div class="flex justify-end gap-2">
				<button type="button" class="btn" onclick={() => (draftOpen = false)}
					>{t('common.cancel')}</button
				>
				<button class="btn btn-primary" disabled={saving}>{t('common.save')}</button>
			</div>
		</form>
	{/if}
</Modal>
