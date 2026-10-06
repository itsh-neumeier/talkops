<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import {
		api,
		type Extension,
		type PhoneNumber,
		type Preset,
		type TrunkAccount,
		type TrunkDetail
	} from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import NumberDestination from '#lib/components/NumberDestination.svelte';
	import { loadTargets } from '#lib/destinations.svelte.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { presetHint, presetLabel, presetNotes, statusClass, statusKey } from '#lib/presets.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let trunk = $state<TrunkDetail | null>(null);
	let preset = $state<Preset | undefined>(undefined);
	let extensions = $state<Extension[]>([]);
	let error = $state('');
	const id = $derived(page.params.id);
	const admin = $derived(hasRole('admin'));

	// edit trunk
	let editOpen = $state(false);
	let edit = $state({
		name: '',
		enabled: true,
		registrar: '',
		proxy: '',
		outbound_proxy: '',
		transport: ''
	});
	// add line / account / number
	let lineOpen = $state(false);
	let line = $state({ e164: '', password: '', username: '', destination_extension_id: '' });
	let accountOpen = $state(false);
	let account = $state<{
		id?: string;
		username: string;
		auth_username: string;
		password: string;
		enabled: boolean;
	}>({ username: '', auth_username: '', password: '', enabled: true });
	let numberOpen = $state(false);
	let number = $state({ e164: '', label: '', account_id: '' });

	async function load() {
		try {
			trunk = await api.get<TrunkDetail>(`/trunks/${id}`);
			const [presets, exts] = await Promise.all([
				api.get<Preset[]>('/presets'),
				api.get<Extension[]>('/extensions')
			]);
			preset = presets.find((p) => p.id === trunk!.preset);
			extensions = exts;
			await loadTargets();
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(() => {
		load();
		const timer = setInterval(async () => {
			try {
				trunk = await api.get<TrunkDetail>(`/trunks/${id}`);
			} catch {
				// keep last state
			}
		}, 10_000);
		return () => clearInterval(timer);
	});

	function openEdit() {
		const o = trunk!.overrides as Record<string, string>;
		edit = {
			name: trunk!.name,
			enabled: trunk!.enabled,
			registrar: o.registrar ?? '',
			proxy: o.proxy ?? '',
			outbound_proxy: o.outbound_proxy ?? '',
			transport: o.transport ?? ''
		};
		editOpen = true;
	}

	async function saveTrunk(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		const overrides: Record<string, unknown> = { ...trunk!.overrides };
		for (const key of ['registrar', 'proxy', 'outbound_proxy', 'transport'] as const) {
			if (edit[key].trim()) overrides[key] = edit[key].trim();
			else delete overrides[key];
		}
		try {
			await api.put(`/trunks/${id}`, {
				name: edit.name,
				preset: trunk!.preset,
				overrides,
				enabled: edit.enabled
			});
			editOpen = false;
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function removeTrunk() {
		if (!confirm(t('common.confirmDelete', { name: trunk!.name }))) return;
		await api.del(`/trunks/${id}`);
		goto('/trunks');
	}

	async function addLine(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		try {
			await api.post(`/trunks/${id}/lines`, {
				e164: line.e164.replace(/[\s/-]/g, ''),
				password: line.password,
				username: line.username || null,
				destination_extension_id: line.destination_extension_id || null
			});
			lineOpen = false;
			line = { e164: '', password: '', username: '', destination_extension_id: '' };
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function openAccount(a?: TrunkAccount) {
		account = a
			? {
					id: a.id,
					username: a.username,
					auth_username: a.auth_username,
					password: '',
					enabled: a.enabled
				}
			: { username: '', auth_username: '', password: '', enabled: true };
		accountOpen = true;
	}

	async function saveAccount(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		const body = {
			username: account.username,
			auth_username: account.auth_username,
			password: account.password || null,
			enabled: account.enabled
		};
		try {
			if (account.id) await api.put(`/trunk-accounts/${account.id}`, body);
			else await api.post(`/trunks/${id}/accounts`, body);
			accountOpen = false;
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function removeAccount(a: TrunkAccount) {
		if (!confirm(t('common.confirmDelete', { name: a.username }))) return;
		await api.del(`/trunk-accounts/${a.id}`);
		await load();
	}

	async function addNumber(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		try {
			await api.post('/numbers', {
				trunk_id: id,
				account_id: number.account_id || null,
				e164: number.e164.replace(/[\s/-]/g, ''),
				label: number.label,
				destination_type: 'none'
			});
			numberOpen = false;
			number = { e164: '', label: '', account_id: '' };
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function removeNumber(n: PhoneNumber) {
		if (!confirm(t('common.confirmDelete', { name: n.e164 }))) return;
		await api.del(`/numbers/${n.id}`);
		await load();
	}

	function stateBadge(a: TrunkAccount) {
		if (!a.state) return ['badge-muted', t('trunks.unknownState')];
		return a.state.state === 'REGED'
			? ['badge-ok', t('trunks.registered')]
			: ['badge-bad', `${t('trunks.notRegistered')} (${a.state.state})`];
	}
</script>

<div class="space-y-4">
	<a class="text-sm text-slate-500 hover:underline" href="/trunks">← {t('common.back')}</a>
	<ErrorBox {error} />
	{#if trunk}
		<div class="flex flex-wrap items-center justify-between gap-2">
			<h1>
				{trunk.name}
				{#if !trunk.enabled}<span class="badge badge-muted">{t('common.disabled')}</span>{/if}
			</h1>
			{#if admin}
				<div class="space-x-1">
					<button class="btn" onclick={openEdit}>{t('common.edit')}</button>
					<button class="btn btn-danger" onclick={removeTrunk}>{t('common.delete')}</button>
				</div>
			{/if}
		</div>
		{#if preset}
			<section class="card space-y-2 text-sm">
				<div class="flex items-center gap-2">
					<span class="font-medium">{presetLabel(preset)}</span>
					<span class="badge {statusClass[preset.status]}">{t(statusKey[preset.status])}</span>
				</div>
				<p>{presetNotes(preset)}</p>
			</section>
		{/if}

		<section class="card space-y-3">
			<div class="flex items-center justify-between">
				<h2>{t('trunks.accounts')}</h2>
				{#if admin && preset}
					{#if preset.credentials.mode === 'per_number'}
						<button class="btn btn-sm btn-primary" onclick={() => (lineOpen = true)}
							>{t('trunks.addLine')}</button
						>
					{:else}
						<button class="btn btn-sm" onclick={() => openAccount()}
							>{t('trunks.addAccount')}</button
						>
					{/if}
				{/if}
			</div>
			<ul class="divide-y divide-slate-100 dark:divide-slate-800">
				{#each trunk.accounts as a (a.id)}
					{@const [cls, label] = stateBadge(a)}
					<li class="flex flex-wrap items-center justify-between gap-2 py-2">
						<div>
							<span class="font-mono">{a.username}</span>
							<span class="badge {cls}">{label}</span>
							{#if a.state?.last_error}<span class="hint">{a.state.last_error}</span>{/if}
						</div>
						{#if admin}
							<div class="space-x-1">
								<button class="btn btn-sm" onclick={() => openAccount(a)}>{t('common.edit')}</button
								>
								<button class="btn btn-sm btn-danger" onclick={() => removeAccount(a)}
									>{t('common.delete')}</button
								>
							</div>
						{/if}
					</li>
				{/each}
			</ul>
		</section>

		<section class="card space-y-3">
			<div class="flex items-center justify-between">
				<h2>{t('trunks.numbers')}</h2>
				{#if admin && preset?.credentials.mode !== 'per_number'}
					<button class="btn btn-sm" onclick={() => (numberOpen = true)}
						>{t('trunks.addNumber')}</button
					>
				{/if}
			</div>
			<table class="table">
				<thead
					><tr
						><th>{t('numbers.number')}</th><th>{t('numbers.label')}</th><th
							>{t('trunks.destination')}</th
						><th></th></tr
					></thead
				>
				<tbody>
					{#each trunk.numbers as n (n.id)}
						<tr>
							<td class="font-mono">{n.e164}</td>
							<td>{n.label}</td>
							<td><NumberDestination number={n} editable={admin} onchange={load} /></td>
							<td class="text-right"
								>{#if admin}<button class="btn btn-sm btn-danger" onclick={() => removeNumber(n)}
										>{t('common.delete')}</button
									>{/if}</td
							>
						</tr>
					{/each}
				</tbody>
			</table>
		</section>
	{/if}
</div>

<Modal title={t('common.edit')} bind:open={editOpen}>
	<form class="space-y-3" onsubmit={saveTrunk}>
		<div>
			<label for="te-name">{t('common.name')}</label><input
				id="te-name"
				class="input"
				bind:value={edit.name}
				required
			/>
		</div>
		<label class="flex items-center gap-2"
			><input type="checkbox" bind:checked={edit.enabled} /> {t('common.enabled')}</label
		>
		<div>
			<label for="te-reg">{t('trunks.registrar')}</label>
			<input
				id="te-reg"
				class="input font-mono"
				bind:value={edit.registrar}
				placeholder={preset?.sip.registrar ?? ''}
			/>
		</div>
		<details>
			<summary class="cursor-pointer text-sm">{t('trunks.advanced')}</summary>
			<div class="mt-2 space-y-3">
				<div>
					<label for="te-proxy">Proxy</label><input
						id="te-proxy"
						class="input font-mono"
						bind:value={edit.proxy}
						placeholder={preset?.sip.proxy ?? ''}
					/>
				</div>
				<div>
					<label for="te-ob">Outbound-Proxy</label><input
						id="te-ob"
						class="input font-mono"
						bind:value={edit.outbound_proxy}
						placeholder={preset?.sip.outbound_proxy ?? ''}
					/>
				</div>
				<div>
					<label for="te-tr">{t('trunks.transport')}</label>
					<select id="te-tr" class="input" bind:value={edit.transport}>
						<option value="">{preset?.sip.transport?.toUpperCase()} *</option>
						<option value="udp">UDP</option><option value="tcp">TCP</option><option value="tls"
							>TLS</option
						>
					</select>
				</div>
			</div>
		</details>
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (editOpen = false)}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary">{t('common.save')}</button>
		</div>
	</form>
</Modal>

<Modal title={t('trunks.addLine')} bind:open={lineOpen}>
	<form class="space-y-3" onsubmit={addLine}>
		<ErrorBox {error} />
		{#if preset}<p class="hint">
				{t('trunks.lineHint', {
					template: presetHint(preset) || preset.credentials.username_template
				})}
			</p>{/if}
		<div>
			<label for="l-e164">{t('numbers.number')}</label>
			<input
				id="l-e164"
				class="input font-mono"
				bind:value={line.e164}
				placeholder="+49891234567"
				required
			/>
			<p class="hint">{t('trunks.e164Hint')}</p>
		</div>
		<div>
			<label for="l-pw">{t('users.password')}</label><input
				id="l-pw"
				class="input"
				type="password"
				bind:value={line.password}
				autocomplete="off"
				required
			/>
		</div>
		<div>
			<label for="l-user">{t('trunks.username')} ({t('common.optional')})</label><input
				id="l-user"
				class="input font-mono"
				bind:value={line.username}
			/>
		</div>
		<div>
			<label for="l-dest">{t('trunks.destination')}</label>
			<select id="l-dest" class="input" bind:value={line.destination_extension_id}>
				<option value="">{t('trunks.noDestination')}</option>
				{#each extensions as e (e.id)}<option value={e.id}>{e.number} {e.display_name}</option
					>{/each}
			</select>
		</div>
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (lineOpen = false)}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary">{t('common.add')}</button>
		</div>
	</form>
</Modal>

<Modal title={account.id ? t('common.edit') : t('trunks.addAccount')} bind:open={accountOpen}>
	<form class="space-y-3" onsubmit={saveAccount}>
		<ErrorBox {error} />
		<div>
			<label for="a-user">{t('trunks.username')}</label>
			<input id="a-user" class="input font-mono" bind:value={account.username} required />
			{#if preset && presetHint(preset)}<p class="hint">{presetHint(preset)}</p>{/if}
		</div>
		<div>
			<label for="a-auth">{t('trunks.authUsername')} ({t('common.optional')})</label><input
				id="a-auth"
				class="input font-mono"
				bind:value={account.auth_username}
			/>
		</div>
		<div>
			<label for="a-pw">{t('users.password')}</label>
			<input
				id="a-pw"
				class="input"
				type="password"
				bind:value={account.password}
				autocomplete="off"
				required={!account.id}
				placeholder={account.id ? t('trunks.passwordKeep') : ''}
			/>
		</div>
		<label class="flex items-center gap-2"
			><input type="checkbox" bind:checked={account.enabled} /> {t('common.enabled')}</label
		>
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (accountOpen = false)}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary">{t('common.save')}</button>
		</div>
	</form>
</Modal>

<Modal title={t('trunks.addNumber')} bind:open={numberOpen}>
	<form class="space-y-3" onsubmit={addNumber}>
		<ErrorBox {error} />
		<div>
			<label for="n-e164">{t('numbers.number')}</label>
			<input
				id="n-e164"
				class="input font-mono"
				bind:value={number.e164}
				placeholder="+49891234567"
				required
			/>
			<p class="hint">{t('trunks.e164Hint')}</p>
		</div>
		<div>
			<label for="n-label">{t('numbers.label')} ({t('common.optional')})</label><input
				id="n-label"
				class="input"
				bind:value={number.label}
			/>
		</div>
		{#if trunk && trunk.accounts.length > 1}
			<div>
				<label for="n-acc">{t('trunks.account')}</label>
				<select id="n-acc" class="input" bind:value={number.account_id}>
					<option value="">—</option>
					{#each trunk.accounts as a (a.id)}<option value={a.id}>{a.username}</option>{/each}
				</select>
			</div>
		{/if}
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (numberOpen = false)}
				>{t('common.cancel')}</button
			>
			<button class="btn btn-primary">{t('common.add')}</button>
		</div>
	</form>
</Modal>
