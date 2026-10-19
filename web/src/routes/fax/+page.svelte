<script lang="ts">
	import { onMount } from 'svelte';
	import { api, ApiError, type PhoneNumber } from '#lib/api.ts';
	import { faxStorageMessageKey, type Fax } from '#lib/fax.ts';
	import { can } from '#lib/session.svelte.ts';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import PageHeader from '#lib/components/PageHeader.svelte';
	import Tabs from '#lib/components/Tabs.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	let tab = $state('outbox');
	let faxes = $state<Fax[]>([]);
	let numbers = $state<PhoneNumber[]>([]);
	let enabled = $state(false);
	let destination = $state('');
	let number = $state('');
	let files = $state<File[]>([]);
	let error = $state('');
	let busy = $state(false);
	let preview = $state<Fax | null>(null);
	let template = $state({ header: '', footer: '' });
	let templateOpen = $state(false);
	function faxError(e: unknown) {
		const key = e instanceof ApiError ? faxStorageMessageKey(e.code) : null;
		return key ? t(key) : errorMessage(e);
	}
	const tabs = $derived([
		...(can('fax.send') ? [{ id: 'send', label: t('fax.send') }] : []),
		{ id: 'outbox', label: t('fax.outbox') },
		{ id: 'inbox', label: t('fax.inbox') }
	]);
	const rows = $derived(
		faxes.filter((f) => f.direction === (tab === 'inbox' ? 'inbound' : 'outbound'))
	);
	async function refresh() {
		faxes = await api.get<Fax[]>('/faxes');
		if (preview) preview = faxes.find((f) => f.id === preview?.id) ?? null;
	}
	onMount(() => {
		let stopped = false;
		void (async () => {
			try {
				const options = await api.get<{ enabled: boolean; numbers: PhoneNumber[] }>(
					'/faxes/options'
				);
				enabled = options.enabled;
				numbers = options.numbers;
				number = numbers[0]?.id ?? '';
				await refresh();
			} catch (e) {
				if (!stopped) error = errorMessage(e);
			}
		})();
		const timer = setInterval(() => {
			if (!stopped)
				void refresh().catch((e) => {
					error = errorMessage(e);
				});
		}, 3000);
		return () => {
			stopped = true;
			clearInterval(timer);
		};
	});
	async function encode(file: File): Promise<string> {
		const bytes = new Uint8Array(await file.arrayBuffer());
		let value = '';
		for (let i = 0; i < bytes.length; i += 8192)
			value += String.fromCharCode(...bytes.subarray(i, i + 8192));
		return btoa(value);
	}
	async function prepare(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = '';
		try {
			if (
				!files.length ||
				files.length > 10 ||
				files.reduce((sum, f) => sum + f.size, 0) > 20 * 1024 * 1024
			)
				throw new Error(t('fax.fileLimit'));
			preview = await api.post<Fax>('/faxes', {
				destination,
				number_id: number || null,
				files: await Promise.all(files.map(async (f) => ({ name: f.name, data: await encode(f) })))
			});
			await refresh();
		} catch (e) {
			error = faxError(e);
		} finally {
			busy = false;
		}
	}
	async function action(f: Fax, kind: string) {
		busy = true;
		error = '';
		try {
			if (kind === 'delete') {
				if (!confirm(t('fax.confirmDelete'))) return;
				await api.del(`/faxes/${f.id}`);
			} else {
				if (kind === 'resend' && !confirm(t('fax.confirmResend'))) return;
				const result = await api.post<Fax>(`/faxes/${f.id}/${kind}`);
				if (kind === 'resend') preview = result;
				if (kind === 'submit') {
					preview = null;
					tab = 'outbox';
				}
			}
			await refresh();
		} catch (e) {
			error = faxError(e);
		} finally {
			busy = false;
		}
	}
	async function editTemplate() {
		try {
			template = await api.get('/faxes/template');
			templateOpen = true;
		} catch (e) {
			error = errorMessage(e);
		}
	}
	async function saveTemplate() {
		try {
			await api.put('/faxes/template', template);
			templateOpen = false;
		} catch (e) {
			error = errorMessage(e);
		}
	}
	async function resetTemplate() {
		try {
			await api.del('/faxes/template');
			template = await api.get('/faxes/template');
		} catch (e) {
			error = errorMessage(e);
		}
	}
	const active = (f: Fax) => ['preparing', 'queued', 'dialing', 'sending'].includes(f.state);
</script>

<PageHeader title={t('fax.title')} />
<ErrorBox {error} />
{#if !enabled}<p class="card p-4">{t('fax.disabled')}</p>{/if}
<Tabs {tabs} bind:value={tab} />
{#if tab === 'send'}
	<form class="card mt-4 space-y-4 p-4" onsubmit={prepare}>
		<p>{t('fax.fileHint')}</p>
		<label class="block"
			>{t('fax.recipient')}<input
				class="input"
				bind:value={destination}
				required
				pattern={'\\+[1-9][0-9]{1,14}'}
				placeholder="+49123456"
			/></label
		>
		<label class="block"
			>{t('fax.sender')}<select class="input" bind:value={number} required
				>{#each numbers as n (n.id)}<option value={n.id}>{n.e164} {n.label}</option>{/each}</select
			></label
		>
		<label class="block"
			>{t('fax.attachments')}<input
				class="input"
				type="file"
				multiple
				accept=".pdf,.png,.jpg,.jpeg,.tif,.tiff,.bmp,.webp"
				onchange={(e) => (files = Array.from(e.currentTarget.files ?? []))}
				required
			/></label
		>
		<ul>
			{#each files as f, i (i)}<li>{i + 1}. {f.name}</li>{/each}
		</ul>
		<div class="flex flex-wrap gap-2">
			<button class="btn btn-primary" disabled={busy || !enabled || !number}
				>{t('fax.prepare')}</button
			>
			{#if can('fax.templates.edit')}<button type="button" class="btn" onclick={editTemplate}
					>{t('fax.editTemplate')}</button
				>{/if}
		</div>
	</form>
{:else}
	<div class="card scroll-x mt-4">
		<table class="w-full">
			<thead
				><tr
					><th>{t('fax.time')}</th><th>{t('fax.sender')}</th><th>{t('fax.recipient')}</th><th
						>{t('fax.pages')}</th
					><th>{t('fax.status')}</th><th>{t('fax.actions')}</th></tr
				></thead
			><tbody>
				{#each rows as f (f.id)}<tr
						><td>{new Date(f.created_at).toLocaleString()}</td><td>{f.sender_email || f.source}</td
						><td>{f.destination}</td><td>{f.pages}</td><td
							><span class="badge-muted">{t(`fax.state.${f.state}` as MessageKey)}</span>
							<div class="text-xs">{f.result}</div></td
						>
						<td
							><div class="flex flex-wrap gap-2">
								<button class="btn" onclick={() => (preview = f)}>{t('fax.preview')}</button>
								{#if can('fax.send') && f.direction === 'outbound'}
									{#if active(f) || f.state === 'draft'}<button
											class="btn"
											disabled={busy}
											onclick={() => action(f, 'cancel')}>{t('fax.cancel')}</button
										>
									{:else}<button class="btn" disabled={busy} onclick={() => action(f, 'resend')}
											>{t('fax.resend')}</button
										>{/if}
								{/if}
								{#if can('fax.delete') && !active(f)}<button
										class="btn btn-danger"
										disabled={busy}
										onclick={() => action(f, 'delete')}>{t('fax.delete')}</button
									>{/if}
							</div></td
						></tr
					>
				{:else}<tr><td colspan="6" class="p-4">{t('fax.empty')}</td></tr>{/each}
			</tbody>
		</table>
	</div>
{/if}
<Modal
	bind:open={
		() => preview !== null,
		(open) => {
			if (!open) preview = null;
		}
	}
	title={t('fax.preview')}
>
	{#if preview}
		<p>{preview.source} → {preview.destination} · {preview.pages} {t('fax.pages')}</p>
		<p>{t(`fax.state.${preview.state}` as MessageKey)} {preview.result}</p>
		{#if preview.pages > 0}<iframe
				title={t('fax.preview')}
				class="mt-3 h-[60dvh] w-full"
				src={`/api/v1/faxes/${preview.id}/document`}
			></iframe>
			<a
				class="btn mt-2"
				href={`/api/v1/faxes/${preview.id}/document`}
				target="_blank"
				rel="noreferrer">{t('fax.download')}</a
			>{/if}
		{#if preview.state === 'draft' && can('fax.send')}<button
				class="btn btn-primary mt-2"
				disabled={busy}
				onclick={() => preview && action(preview, 'submit')}>{t('fax.sendNow')}</button
			>{/if}
	{/if}
</Modal>
<Modal bind:open={templateOpen} title={t('fax.editTemplate')}>
	<p>{t('fax.placeholders')}</p>
	<label class="block"
		>{t('fax.header')}<input class="input" bind:value={template.header} maxlength="180" /></label
	>
	<label class="block"
		>{t('fax.footer')}<input class="input" bind:value={template.footer} maxlength="180" /></label
	>
	<button class="btn btn-primary mt-3" onclick={saveTemplate}>{t('common.save')}</button>
	<button class="btn mt-3" onclick={resetTemplate}>{t('fax.resetTemplate')}</button>
</Modal>
