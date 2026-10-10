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
		type PhoneMedia,
		type PhoneModel,
		type PhonebookSection
	} from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	const id = $derived(page.params.id);
	let phone = $state<PhoneDetail | null>(null);
	let models = $state<PhoneModel[]>([]);
	let extensions = $state<Extension[]>([]);
	let media = $state<PhoneMedia[]>([]);
	let sections = $state<PhonebookSection[]>([]);
	let form = $state({
		name: '',
		mac: '',
		model: '',
		ringtone_id: '',
		wallpaper_id: '',
		phonebook_sections: [] as string[]
	});
	/** Label and display name per account (device id), as edited. */
	let texts = $state<Record<string, { phone_label: string; phone_display_name: string }>>({});
	let keys = $state<LineKey[]>([]);
	let error = $state('');
	let message = $state('');
	let config = $state('');
	let configOpen = $state(false);

	const model = $derived(models.find((m) => m.id === form.model));
	const keyTypes: KeyType[] = ['line', 'blf', 'speed_dial', 'none'];
	const MAX_SECTIONS = 3;
	const ringtones = $derived(media.filter((m) => m.kind === 'ringtone'));
	const wallpapers = $derived(media.filter((m) => m.kind === 'wallpaper'));

	function toggleSection(sid: string, on: boolean) {
		form.phonebook_sections = on
			? [...form.phonebook_sections, sid]
			: form.phonebook_sections.filter((s) => s !== sid);
	}

	/** Ringtones larger than the model accepts are not offered. */
	const fits = (m: PhoneMedia) => !model || m.size_bytes <= model.ringtone_max_kb * 1024;

	async function load() {
		try {
			[phone, models, extensions, media, sections] = await Promise.all([
				api.get<PhoneDetail>(`/phones/${id}`),
				api.get<PhoneModel[]>('/phone-models'),
				api.get<Extension[]>('/extensions'),
				api.get<PhoneMedia[]>('/phone-media').catch(() => []),
				api.get<PhonebookSection[]>('/phonebook-sections')
			]);
			form = {
				name: phone.name,
				mac: phone.mac,
				model: phone.model,
				ringtone_id: phone.ringtone_id ?? '',
				wallpaper_id: phone.wallpaper_id ?? '',
				phonebook_sections: [...phone.phonebook_sections]
			};
			texts = Object.fromEntries(
				phone.accounts.map((a) => [
					a.device_id,
					{ phone_label: a.phone_label, phone_display_name: a.phone_display_name }
				])
			);
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
			await api.put(`/phones/${id}`, {
				...form,
				ringtone_id: model?.ringtone_max_kb ? form.ringtone_id || null : null,
				wallpaper_id: model?.wallpaper ? form.wallpaper_id || null : null,
				line_keys: keys
			});
			for (const a of phone?.accounts ?? []) {
				const edited = texts[a.device_id];
				if (
					edited &&
					(edited.phone_label !== a.phone_label ||
						edited.phone_display_name !== a.phone_display_name)
				)
					await api.put(`/phones/${id}/accounts/${a.device_id}`, edited);
			}
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
					? formatDateTime(phone.last_seen_at)
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
				<p class="hint">{t('phones.accountTextsHint')}</p>
				<ul class="divide-y divide-slate-100 dark:divide-slate-800">
					{#each phone.accounts as a (a.device_id)}
						<li class="grid gap-2 py-2 sm:grid-cols-3 sm:items-end">
							<div>
								<span class="text-slate-500">{t('phones.account')} {a.account_index}:</span>
								<a class="font-mono hover:underline" href="/extensions/{a.extension_id}"
									>{a.extension_number}</a
								>
								{a.display_name}
							</div>
							{#if texts[a.device_id]}
								<div>
									<label for="lbl-{a.device_id}">{t('phones.accountLabel')}</label>
									<input
										id="lbl-{a.device_id}"
										class="input"
										maxlength="32"
										placeholder={a.extension_number}
										disabled={!hasRole('admin')}
										bind:value={texts[a.device_id].phone_label}
									/>
								</div>
								<div>
									<label for="dn-{a.device_id}">{t('phones.accountDisplayName')}</label>
									<input
										id="dn-{a.device_id}"
										class="input"
										maxlength="64"
										placeholder={a.display_name}
										disabled={!hasRole('admin')}
										bind:value={texts[a.device_id].phone_display_name}
									/>
								</div>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}
		</section>

		<section class="card space-y-3">
			<h2>{t('phones.personalize')}</h2>
			<div class="grid gap-3 sm:grid-cols-2">
				<div>
					<label for="ph-ring">{t('media.ringtone')}</label>
					{#if model?.ringtone_max_kb}
						<select id="ph-ring" class="input" bind:value={form.ringtone_id}>
							<option value="">{t('phones.phoneDefault')}</option>
							{#each ringtones as m (m.id)}
								<option value={m.id} disabled={!fits(m)}
									>{m.name}{fits(m) ? '' : ` (${t('phones.tooLarge')})`}</option
								>
							{/each}
						</select>
					{:else}
						<p class="text-sm text-slate-500">{t('phones.notSupported')}</p>
					{/if}
				</div>
				<div>
					<label for="ph-wall">{t('media.wallpaper')}</label>
					{#if model?.wallpaper}
						<select id="ph-wall" class="input" bind:value={form.wallpaper_id}>
							<option value="">{t('phones.phoneDefault')}</option>
							{#each wallpapers as m (m.id)}<option value={m.id}>{m.name}</option>{/each}
						</select>
					{:else}
						<p class="text-sm text-slate-500">{t('phones.notSupported')}</p>
					{/if}
				</div>
			</div>
			<p class="hint">{t('phones.personalizeHint')}</p>
			<fieldset>
				<legend class="mb-1 text-sm text-slate-600 dark:text-slate-300">
					{t('phones.phonebookSections')}
				</legend>
				{#if sections.length === 0}
					<p class="text-sm text-slate-500">{t('phones.noSections')}</p>
				{:else}
					<div class="flex flex-wrap gap-x-4 gap-y-1">
						{#each sections as sec (sec.id)}
							{@const on = form.phonebook_sections.includes(sec.id)}
							<label class="flex items-center gap-2 font-normal">
								<input
									type="checkbox"
									checked={on}
									disabled={!on && form.phonebook_sections.length >= MAX_SECTIONS}
									onchange={(e) => toggleSection(sec.id, e.currentTarget.checked)}
								/>
								{sec.name}
							</label>
						{/each}
					</div>
				{/if}
				<p class="hint">{t('phones.phonebookSectionsHint')}</p>
			</fieldset>
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
