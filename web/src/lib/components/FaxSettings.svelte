<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type PhoneNumber } from '#lib/api.ts';
	import type { FaxConfig, FaxEndpoint, FaxWhitelist } from '#lib/fax.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import ErrorBox from './ErrorBox.svelte';
	import Modal from './Modal.svelte';
	let cfg = $state<FaxConfig | null>(null);
	let numbers = $state<PhoneNumber[]>([]);
	let users = $state<{ id: string; name: string }[]>([]);
	let endpoints = $state<FaxEndpoint[]>([]);
	let whitelist = $state<FaxWhitelist[]>([]);
	let password = $state('');
	let hasPassword = $state(false);
	let error = $state('');
	let saved = $state(false);
	let saving = $state(false);
	let editing = $state<FaxEndpoint | null>(null);
	let inputLog = $state<{
		receipts: { sender: string; result: string; created_at: string }[];
		deliveries: { recipient: string; kind: string; sent: boolean; attempts: number }[];
	}>({ receipts: [], deliveries: [] });
	const trunks = $derived([...new Set(numbers.map((n) => n.trunk_id))]);
	onMount(() => {
		void load();
	});
	async function load() {
		try {
			const [view, list, allowed] = await Promise.all([
				api.get<{
					config: FaxConfig;
					has_password: boolean;
					numbers: PhoneNumber[];
					users: { id: string; name: string }[];
				}>('/settings/fax'),
				api.get<FaxEndpoint[]>('/fax-endpoints'),
				api.get<FaxWhitelist[]>('/settings/fax/whitelist')
			]);
			cfg = view.config;
			hasPassword = view.has_password;
			numbers = view.numbers;
			users = view.users;
			endpoints = list;
			whitelist = allowed;
			inputLog = await api.get('/settings/fax/input-log');
		} catch (e) {
			error = errorMessage(e);
		}
	}
	async function save(event: SubmitEvent) {
		event.preventDefault();
		saving = true;
		saved = false;
		error = '';
		try {
			await api.put('/settings/fax', { config: cfg, password: password || null });
			await api.put('/settings/fax/whitelist', whitelist);
			password = '';
			saved = true;
			await load();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			saving = false;
		}
	}
	function addEndpoint() {
		editing = {
			id: crypto.randomUUID(),
			name: '',
			owner_id: users[0]?.id ?? '',
			send_users: [],
			receive_users: [],
			reception_email: '',
			enabled: true
		};
	}
	async function saveEndpoint(event: SubmitEvent) {
		event.preventDefault();
		try {
			await api.put('/fax-endpoints', editing);
			editing = null;
			await load();
		} catch (e) {
			error = errorMessage(e);
		}
	}
</script>

<ErrorBox {error} />
{#if saved}<p role="status" class="badge-ok">{t('fax.saved')}</p>{/if}
{#if cfg}
	<form class="space-y-4" onsubmit={save}>
		<section class="card space-y-3 p-4">
			<h2>{t('fax.title')}</h2>
			<label class="flex gap-2"
				><input type="checkbox" bind:checked={cfg.enabled} />{t('fax.enable')}</label
			>
			<label class="block"
				>{t('fax.globalNumber')}<select class="input" bind:value={cfg.global_number_id}
					><option value={null}>{t('dest.none')}</option>{#each numbers as n (n.id)}<option
							value={n.id}>{n.e164} {n.label}</option
						>{/each}</select
				></label
			>
			<label class="flex gap-2"
				><input type="checkbox" bind:checked={cfg.always_global} />{t('fax.alwaysGlobal')}</label
			>
			<label class="block"
				>{t('fax.header')}<input class="input" bind:value={cfg.header} maxlength="180" /></label
			>
			<label class="block"
				>{t('fax.footer')}<input class="input" bind:value={cfg.footer} maxlength="180" /></label
			>
			<p class="text-sm">{t('fax.placeholders')}</p>
			<label class="block"
				>{t('fax.retention')}<input
					class="input"
					type="number"
					min="1"
					max="3650"
					bind:value={cfg.retention_days}
				/></label
			>
			{#each trunks as trunk (trunk)}<label class="block"
					>{t('fax.transport')}: {numbers
						.filter((n) => n.trunk_id === trunk)
						.map((n) => n.e164)
						.join(', ')}
					<select
						class="input"
						value={cfg.trunk_modes[trunk] ?? 'preferred'}
						onchange={(e) => {
							if (cfg) cfg.trunk_modes[trunk] = e.currentTarget.value;
						}}
						><option value="preferred">{t('fax.preferred')}</option><option value="audio"
							>{t('fax.audio')}</option
						><option value="required">{t('fax.required')}</option></select
					>
				</label>{/each}
		</section>
		<section class="card space-y-3 p-4">
			<h2>{t('fax.mailInput')}</h2>
			<p>{t('fax.mailRules')}</p>
			<label class="flex gap-2"
				><input type="checkbox" bind:checked={cfg.imap_enabled} />{t('fax.enableMail')}</label
			>
			<label class="block"
				>{t('fax.mailAddress')}<input
					class="input"
					type="email"
					bind:value={cfg.faxin_address}
				/></label
			>
			<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
				<label>{t('fax.imapHost')}<input class="input" bind:value={cfg.imap_host} /></label>
				<label
					>{t('fax.imapPort')}<input
						class="input"
						type="number"
						min="1"
						max="65535"
						bind:value={cfg.imap_port}
					/></label
				>
				<label
					>{t('fax.imapUser')}<input
						class="input"
						bind:value={cfg.imap_username}
						autocomplete="off"
					/></label
				>
				<label
					>{t('fax.imapPassword')}<input
						class="input"
						type="password"
						bind:value={password}
						placeholder={hasPassword ? '••••••••' : ''}
						autocomplete="new-password"
					/></label
				>
				<label>{t('fax.imapFolder')}<input class="input" bind:value={cfg.imap_folder} /></label>
			</div>
			<label class="flex gap-2"
				><input type="checkbox" bind:checked={cfg.trusted_mailbox} />{t(
					'fax.trustedMailbox'
				)}</label
			>
			<h3>{t('fax.whitelist')}</h3>
			<p>{t('fax.whitelistHint')}</p>
			{#each whitelist as entry, i (i)}<div
					class="grid grid-cols-1 items-end gap-2 @lg:grid-cols-2"
				>
					<label
						>{t('fax.email')}<input
							class="input"
							type="email"
							bind:value={entry.email}
							required
						/></label
					>
					<label
						>{t('fax.owner')}<select class="input" bind:value={entry.user_id} required
							>{#each users as u (u.id)}<option value={u.id}>{u.name}</option>{/each}</select
						></label
					>
					<label><input type="checkbox" bind:checked={entry.enabled} /> {t('fax.active')}</label>
					<button
						type="button"
						class="btn btn-danger"
						onclick={() => (whitelist = whitelist.filter((_, j) => j !== i))}
						>{t('fax.delete')}</button
					>
				</div>{/each}
			<button
				type="button"
				class="btn"
				onclick={() =>
					(whitelist = [...whitelist, { email: '', user_id: users[0]?.id ?? '', enabled: true }])}
				>{t('fax.addSender')}</button
			>
		</section>
		<button class="btn btn-primary" disabled={saving}>{t('common.save')}</button>
	</form>
	<section class="card mt-4 space-y-3 p-4">
		<h2>{t('fax.endpoints')}</h2>
		<p>{t('fax.endpointHint')}</p>
		{#each endpoints as endpoint (endpoint.id)}<div class="flex items-center justify-between gap-3">
				<span>{endpoint.name} {endpoint.enabled ? '' : t('fax.disabled')}</span><button
					class="btn"
					onclick={() => (editing = structuredClone($state.snapshot(endpoint)))}
					>{t('fax.edit')}</button
				>
			</div>{/each}
		<button class="btn" onclick={addEndpoint}>{t('fax.addEndpoint')}</button>
	</section>
{/if}
<section class="card mt-4 space-y-3 p-4">
	<h2>{t('fax.inputLog')}</h2>
	{#each inputLog.receipts as row, i (i)}<p>
			{new Date(row.created_at).toLocaleString()} · {row.sender} · {row.result}
		</p>{:else}<p>{t('fax.empty')}</p>{/each}
	<h3>{t('fax.mailReports')}</h3>
	{#each inputLog.deliveries as row, i (i)}<p>
			{row.recipient} · {row.sent ? t('fax.mailDelivered') : t('fax.mailPending')} · {row.attempts}/5
		</p>{/each}
</section>
<Modal
	bind:open={
		() => editing !== null,
		(open) => {
			if (!open) editing = null;
		}
	}
	title={t('fax.endpoints')}
>
	{#if editing}<form class="space-y-3" onsubmit={saveEndpoint}>
			<label class="block"
				>{t('fax.name')}<input
					class="input"
					bind:value={editing.name}
					required
					maxlength="100"
				/></label
			>
			<label class="block"
				>{t('fax.owner')}<select class="input" bind:value={editing.owner_id} required
					>{#each users as u (u.id)}<option value={u.id}>{u.name}</option>{/each}</select
				></label
			>
			<label class="block"
				>{t('fax.sendUsers')}<select class="input" multiple bind:value={editing.send_users}
					>{#each users as u (u.id)}<option value={u.id}>{u.name}</option>{/each}</select
				></label
			>
			<label class="block"
				>{t('fax.receiveUsers')}<select class="input" multiple bind:value={editing.receive_users}
					>{#each users as u (u.id)}<option value={u.id}>{u.name}</option>{/each}</select
				></label
			>
			<label class="block"
				>{t('fax.receptionEmail')}<input
					class="input"
					type="email"
					bind:value={editing.reception_email}
				/></label
			>
			<label><input type="checkbox" bind:checked={editing.enabled} /> {t('fax.active')}</label>
			<button class="btn btn-primary">{t('common.save')}</button>
		</form>{/if}
</Modal>
