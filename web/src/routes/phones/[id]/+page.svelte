<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import {
		api,
		getText,
		type Extension,
		type KeyType,
		type LineKey,
		type PhoneDetail,
		type PhoneModel
	} from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	const id = $derived(page.params.id);
	let phone = $state<PhoneDetail | null>(null);
	let models = $state<PhoneModel[]>([]);
	let extensions = $state<Extension[]>([]);
	let form = $state({ name: '', mac: '', model: '' });
	let keys = $state<LineKey[]>([]);
	let error = $state('');
	let message = $state('');
	let config = $state('');
	let configOpen = $state(false);

	const model = $derived(models.find((m) => m.id === form.model));
	const keyTypes: KeyType[] = ['line', 'blf', 'speed_dial', 'none'];

	async function load() {
		try {
			[phone, models, extensions] = await Promise.all([
				api.get<PhoneDetail>(`/phones/${id}`),
				api.get<PhoneModel[]>('/phone-models'),
				api.get<Extension[]>('/extensions')
			]);
			form = { name: phone.name, mac: phone.mac, model: phone.model };
			keys = phone.line_keys.map((k) => ({ ...k }));
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	/** Key layout row for every key of the model; unconfigured keys show the default. */
	const rows = $derived(
		Array.from({ length: model?.line_keys ?? 0 }, (_, i) => i + 1).map(
			(n) => keys.find((k) => k.key === n) ?? null
		)
	);

	function defaultLabel(n: number) {
		const a = phone?.accounts.find((a) => a.account_index === n);
		return a ? `${t('phones.keyLine')} ${a.extension_number}` : t('phones.keyUnused');
	}

	function configure(n: number) {
		keys = [...keys, { key: n, type: 'blf', value: '', label: '', account: 1 }];
	}

	function reset(n: number) {
		keys = keys.filter((k) => k.key !== n);
	}

	function pickExtension(k: LineKey, number: string) {
		k.value = number;
		const ext = extensions.find((e) => e.number === number);
		if (ext && !k.label) k.label = ext.display_name;
	}

	async function save() {
		error = '';
		message = '';
		try {
			await api.put(`/phones/${id}`, { ...form, line_keys: keys });
			message = t('phones.savedResync');
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function resync() {
		error = '';
		message = '';
		try {
			const r = await api.post<{ notified: number }>(`/phones/${id}/resync`);
			message = r.notified > 0 ? t('phones.resyncSent') : t('phones.resyncOffline');
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function showConfig() {
		try {
			config = await getText(`/phones/${id}/config`);
			configOpen = true;
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove() {
		if (!phone || !confirm(t('common.confirmDelete', { name: phone.name }))) return;
		try {
			await api.del(`/phones/${id}`);
			goto('/phones');
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<div class="space-y-4">
	<a class="text-sm text-slate-500 hover:underline" href="/phones">← {t('common.back')}</a>
	<ErrorBox {error} />
	{#if message}<p class="text-sm text-emerald-700 dark:text-emerald-400">{message}</p>{/if}
	{#if phone}
		<div class="flex flex-wrap items-center justify-between gap-2">
			<h1>{phone.name} <span class="text-base font-normal text-slate-500">{model?.name}</span></h1>
			{#if hasRole('admin')}
				<div class="space-x-1">
					<button class="btn" onclick={resync}>{t('phones.resync')}</button>
					<button class="btn" onclick={showConfig}>{t('phones.showConfig')}</button>
					<button class="btn btn-danger" onclick={remove}>{t('common.delete')}</button>
				</div>
			{/if}
		</div>

		<section class="card space-y-3">
			<div class="grid gap-3 sm:grid-cols-3">
				<div>
					<label for="ph-name">{t('common.name')}</label>
					<input id="ph-name" class="input" bind:value={form.name} />
				</div>
				<div>
					<label for="ph-mac">{t('phones.mac')}</label>
					<input id="ph-mac" class="input font-mono" bind:value={form.mac} />
				</div>
				<div>
					<label for="ph-model">{t('phones.model')}</label>
					<select id="ph-model" class="input" bind:value={form.model}>
						{#each models as m (m.id)}<option value={m.id}>{m.name}</option>{/each}
					</select>
				</div>
			</div>
			<p class="text-sm text-slate-500">
				{t('phones.lastSeen')}: {phone.last_seen_at
					? new Date(phone.last_seen_at).toLocaleString()
					: t('phones.neverSeen')}{phone.last_ip ? ` · ${phone.last_ip}` : ''}{phone.last_firmware
					? ` · ${t('phones.firmware')} ${phone.last_firmware}`
					: ''}
			</p>
		</section>

		<section class="card space-y-2">
			<h2>{t('phones.accounts')}</h2>
			{#if phone.accounts.length === 0}
				<p class="text-sm text-slate-500">{t('phones.noAccounts')}</p>
			{:else}
				<ul class="divide-y divide-slate-100 dark:divide-slate-800">
					{#each phone.accounts as a (a.device_id)}
						<li class="py-2">
							<span class="text-slate-500">{t('phones.account')} {a.account_index}:</span>
							<a class="font-mono hover:underline" href="/extensions/{a.extension_id}"
								>{a.extension_number}</a
							>
							{a.display_name}
						</li>
					{/each}
				</ul>
			{/if}
		</section>

		{#if model && model.line_keys > 0}
			<section class="card space-y-3">
				<h2>{t('phones.keys')}</h2>
				<p class="hint">{t('phones.keysHint')}</p>
				<div class="overflow-x-auto">
					<table class="table">
						<thead>
							<tr
								><th>#</th><th>{t('phones.keyType')}</th><th>{t('phones.keyValue')}</th><th
									>{t('phones.keyLabel')}</th
								><th>{t('phones.account')}</th><th></th></tr
							>
						</thead>
						<tbody>
							{#each rows as k, i (i)}
								<tr>
									<td class="font-mono">{i + 1}</td>
									{#if k}
										<td>
											<select class="input" bind:value={k.type}>
												{#each keyTypes as kt (kt)}<option value={kt}>{t(`keytype.${kt}`)}</option
													>{/each}
											</select>
										</td>
										<td>
											{#if k.type === 'blf'}
												<select
													class="input"
													value={k.value}
													onchange={(e) => pickExtension(k, e.currentTarget.value)}
												>
													<option value="">—</option>
													{#each extensions as e (e.id)}<option value={e.number}
															>{e.number} {e.display_name}</option
														>{/each}
												</select>
											{:else if k.type === 'speed_dial'}
												<input class="input font-mono" bind:value={k.value} inputmode="tel" />
											{/if}
										</td>
										<td><input class="input" bind:value={k.label} maxlength="32" /></td>
										<td>
											<input
												class="input w-20"
												type="number"
												min="1"
												max={model.accounts}
												bind:value={k.account}
											/>
										</td>
										<td
											><button class="btn btn-sm" onclick={() => reset(i + 1)}
												>{t('phones.keyReset')}</button
											></td
										>
									{:else}
										<td colspan="4" class="text-sm text-slate-500">{defaultLabel(i + 1)}</td>
										<td
											><button class="btn btn-sm" onclick={() => configure(i + 1)}
												>{t('common.edit')}</button
											></td
										>
									{/if}
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</section>
		{/if}

		{#if hasRole('admin')}
			<div class="flex justify-end">
				<button class="btn btn-primary" onclick={save}>{t('common.save')}</button>
			</div>
		{/if}
	{/if}
</div>

<Modal title={t('phones.showConfig')} bind:open={configOpen}>
	<pre class="overflow-x-auto rounded bg-slate-100 p-2 text-xs dark:bg-slate-800">{config}</pre>
</Modal>
