<script lang="ts">
	import PhoneVendorSettings from '#lib/components/PhoneVendorSettings.svelte';
	import PhoneNativeKeys from '#lib/components/PhoneNativeKeys.svelte';
	import PhoneCloneDialog from '#lib/components/PhoneCloneDialog.svelte';
	import PhoneProvisioningEditor from '#lib/components/PhoneProvisioningEditor.svelte';
	import { configurationDiff } from '#lib/provisioning.ts';
	import type { PhoneProvisioning } from '#lib/api.ts';
	import PageHeader from '#lib/components/PageHeader.svelte';
	import PhoneNetworkCard from '#lib/components/PhoneNetworkCard.svelte';
	import PhoneSettingsEditor from '#lib/components/PhoneSettingsEditor.svelte';
	import PhoneKeyEditor from '#lib/components/PhoneKeyEditor.svelte';
	import PhoneGraphic from '#lib/components/PhoneGraphic.svelte';
	import { assignExtension } from '#lib/phoneAssign.ts';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import {
		api,
		getText,
		type Extension,
		type Contact,
		type KeyType,
		type LineKey,
		type PhoneDetail,
		type PhoneMedia,
		type PhoneModel,
		type PhoneSettingsView,
		type PhonebookSection
	} from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	const id = $derived(page.params.id);
	let phone = $state<PhoneDetail | null>(null);
	let cloneOpen = $state(false),
		cloneSource = $state('');
	let models = $state<PhoneModel[]>([]);
	let extensions = $state<Extension[]>([]);
	let media = $state<PhoneMedia[]>([]);
	let sections = $state<PhonebookSection[]>([]);
	let contacts = $state<Contact[]>([]);
	let saving = $state(false);
	let form = $state({
		name: '',
		mac: '',
		model: '',
		ringtone_id: '',
		wallpaper_id: '',
		phonebook_sections: [] as string[],
		settings: {} as Record<string, string>
	});
	/** Comfort settings catalog and the values for all phones. */
	let comfort = $state<PhoneSettingsView | null>(null);
	/** Label and display name per account (device id), as edited. */
	let texts = $state<Record<string, { phone_label: string; phone_display_name: string }>>({});
	let keys = $state<LineKey[]>([]);
	let provisioning = $state<PhoneProvisioning>({});
	let previewOpen = $state(false),
		previewText = $state(''),
		previewDiff = $state('');
	let error = $state('');
	let message = $state('');
	let config = $state('');
	let configOpen = $state(false);
	let qrImage = $state('');

	const baseModel = $derived(models.find((m) => m.id === form.model));
	const model = $derived(
		baseModel
			? { ...baseModel, accounts: provisioning.account_slots ?? baseModel.accounts }
			: undefined
	);
	function changeModel(next: string) {
		if (next === form.model) return;
		form.model = next;
		keys = [];
		provisioning = {
			...provisioning,
			account_slots: undefined,
			parameters: {},
			vendor_settings: {},
			keys: [],
			softkeys: {},
			firmware_profile: '',
			template_id: null
		};
		message = t('vendor.modelChanged');
	}
	const comfortShown = $derived(
		!!model && !!comfort?.catalog.some((s) => s.families.includes(model.family))
	);
	const keyTypes = $derived<KeyType[]>(
		model?.speed_dial_keys === false
			? ['line', 'blf', 'none']
			: ['line', 'blf', 'speed_dial', 'none']
	);
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

	let loadSequence = 0;
	async function load() {
		const sequence = ++loadSequence;
		const requestedId = id;
		try {
			const result = await Promise.all([
				api.get<PhoneDetail>(`/phones/${requestedId}`),
				api.get<PhoneModel[]>('/phone-models'),
				api.get<Extension[]>('/extensions'),
				api.get<PhoneMedia[]>('/phone-media').catch(() => []),
				api.get<PhonebookSection[]>('/phonebook-sections'),
				api.get<Contact[]>('/contacts')
			]);
			const settings = await api.get<PhoneSettingsView>('/phone-settings').catch(() => null);
			if (sequence !== loadSequence || requestedId !== id) return;
			[phone, models, extensions, media, sections, contacts] = result;
			comfort = settings;
			form = {
				name: phone.name,
				mac: phone.mac,
				model: phone.model,
				ringtone_id: phone.ringtone_id ?? '',
				wallpaper_id: phone.wallpaper_id ?? '',
				phonebook_sections: [...phone.phonebook_sections],
				settings: { ...(phone.settings ?? {}) }
			};
			texts = Object.fromEntries(
				phone.accounts.map((a) => [
					a.device_id,
					{ phone_label: a.phone_label, phone_display_name: a.phone_display_name }
				])
			);
			keys = phone.line_keys.map((k) => ({ ...k }));
			provisioning = structuredClone($state.snapshot(phone.provisioning ?? {}));
		} catch (err) {
			if (sequence === loadSequence) error = errorMessage(err);
		}
	}
	$effect(() => {
		if (id) {
			phone = null;
			void load();
		}
	});

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

	async function save(withResync = false) {
		if (saving) return;
		saving = true;
		let saved = false;
		error = '';
		message = '';
		try {
			await api.put(`/phones/${id}`, {
				...form,
				ringtone_id: model?.ringtone_max_kb ? form.ringtone_id || null : null,
				wallpaper_id: model?.wallpaper ? form.wallpaper_id || null : null,
				provisioning,
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
			saved = true;
			await load();
			if (withResync) {
				const result = await api.post<{ notified: number }>(`/phones/${id}/resync`, {
					reboot: false
				});
				message = t(result.notified === 0 ? 'phonekeys.savedOffline' : 'phonekeys.savedSynced');
			}
		} catch (err) {
			error = saved
				? `${t('phonekeys.savedResyncFailed')} ${errorMessage(err)}`
				: errorMessage(err);
		} finally {
			saving = false;
		}
	}

	/** Reload the configuration; with `reboot` the phone restarts as well. */
	/** Extension chosen to put on this phone. */
	let assignId = $state('');
	const assignable = $derived(
		extensions.filter((e) => !phone?.accounts.some((a) => a.extension_id === e.id))
	);
	const slotsFree = $derived(!!model && !!phone && phone.accounts.length < model.accounts);

	async function assign() {
		if (!phone || !assignId) return;
		error = '';
		message = '';
		try {
			await assignExtension(phone, model?.family, assignId);
			assignId = '';
			message = t('phones.assigned');
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function unassign(deviceId: string, number: string) {
		if (!confirm(t('phones.unassignConfirm', { number }))) return;
		error = '';
		try {
			await api.del(`/devices/${deviceId}`);
			message = t('phones.unassigned');
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function resync(reboot: boolean) {
		if (reboot && !confirm(t('phones.rebootConfirm'))) return;
		error = '';
		message = '';
		try {
			const r = await api.post<{ notified: number }>(`/phones/${id}/resync`, { reboot });
			message =
				r.notified === 0
					? t('phones.resyncOffline')
					: reboot
						? t('phones.rebootSent')
						: t('phones.resyncSent');
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function preview() {
		try {
			const result = await api.post<{ before: string; after: string }>(`/phones/${id}/preview`, {
				...form,
				ringtone_id: form.ringtone_id || null,
				wallpaper_id: form.wallpaper_id || null,
				line_keys: keys,
				provisioning,
				accounts: texts
			});
			previewText = result.after;
			previewDiff = configurationDiff(result.before, result.after);
			previewOpen = true;
		} catch (err) {
			error = errorMessage(err);
		}
	}
	async function showConfig(artifact?: string, qr = false) {
		try {
			const query = new URLSearchParams();
			if (artifact) query.set('artifact', artifact);
			if (qr) query.set('qr', 'true');
			config = await getText(`/phones/${id}/config?${query}`);
			qrImage = qr ? `data:image/svg+xml,${encodeURIComponent(config)}` : '';
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
	<PageHeader
		back={{ href: '/phones', label: t('nav.phones') }}
		heading={phone ? heading : undefined}
		actions={phone && can('phones.manage') ? actions : undefined}
	/>
	{#snippet heading()}
		{phone!.name}
		<span class="text-base font-normal text-slate-500 dark:text-slate-400">{model?.name}</span>
	{/snippet}
	{#snippet actions()}
		<button
			class="btn"
			onclick={() => {
				cloneSource = id ?? '';
				cloneOpen = true;
			}}>{t('phoneclone.title')}</button
		>
		<button class="btn" onclick={preview}>{t('prov.preview')}</button>
		{#if model?.provisioning_capabilities?.resync !== false}<button
				class="btn"
				title={t('phones.resyncHint')}
				onclick={() => resync(false)}>{t('phones.resync')}</button
			>
			<button class="btn" onclick={() => resync(true)}>{t('phones.reboot')}</button>{/if}
		<button class="btn" onclick={() => showConfig()}>{t('phones.showConfig')}</button>
		<button class="btn btn-danger" onclick={remove}>{t('common.delete')}</button>
	{/snippet}
	<ErrorBox {error} />
	{#if message}<p class="text-sm text-emerald-700 dark:text-emerald-400">{message}</p>{/if}
	{#if phone}
		<PhoneNetworkCard phoneId={phone.id} />
		{#if phone.provisioning_files?.length}
			<section class="card space-y-3">
				<h2>{t('vendor.files')}</h2>
				<p class="hint">{t('vendor.filesHint')}</p>
				{#if phone.provisioning_base}<p>
						{t('vendor.baseAddress')}<code class="block break-all text-xs"
							>{phone.provisioning_base}</code
						>
					</p>{/if}
				<p class="hint">{t('vendor.notTested')} · {t('vendor.manualResync')}</p>
				{#each phone.provisioning_files as file (file.artifact)}
					<div class="min-w-0 rounded-lg border border-line p-3">
						<strong>{file.name}{file.primary ? ' ★' : ''}</strong>
						<code class="block break-all text-xs">{file.url}</code>
						{#if can('phones.manage')}<div class="mt-2 flex flex-wrap gap-2">
								<button type="button" class="btn" onclick={() => showConfig(file.artifact)}
									>{t('phones.showConfig')}</button
								>
								{#if file.name.includes('qr_template') || file.format === 'csv'}<button
										type="button"
										class="btn"
										onclick={() => showConfig(file.artifact, true)}>{t('vendor.showQr')}</button
									>{/if}
							</div>{/if}
					</div>
				{/each}
			</section>
		{/if}
		{#if model?.provisioning_capabilities?.native && baseModel}
			<section class="card space-y-3">
				<h2>{t('vendor.accountSlots')}</h2>
				<p class="hint">{t('vendor.accountSlotsHint')}</p>
				<label for="phone-account-count"
					>{t('vendor.accountSlots')}<input
						id="phone-account-count"
						class="input max-w-40"
						type="number"
						min="1"
						max={baseModel.provisioning_capabilities?.account_capacity_source ===
						'single_account_baseline'
							? 100
							: baseModel.accounts}
						disabled={!can('phones.manage')}
						value={model.accounts}
						oninput={(e) =>
							(provisioning = { ...provisioning, account_slots: Number(e.currentTarget.value) })}
					/></label
				>
			</section>
		{/if}
		<section class="card space-y-3">
			<div class="grid grid-cols-1 gap-3 @xl:grid-cols-3">
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
					<select
						id="ph-model"
						class="input"
						value={form.model}
						onchange={(e) => changeModel(e.currentTarget.value)}
					>
						{#each models as m (m.id)}<option value={m.id}>{m.vendor} {m.name}</option>{/each}
					</select>
					{#if model?.layout}<div class="mt-2 flex items-center gap-2">
							<PhoneGraphic {model} small /><span class="hint">{model.name}</span>
						</div>{/if}
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
				<p class="text-sm text-slate-500">{t('phones.noAccountsYet')}</p>
			{:else}
				<p class="hint">{t('phones.accountTextsHint')}</p>
				<ul class="divide-y divide-slate-100 dark:divide-slate-800">
					{#each phone.accounts as a (a.device_id)}
						<li class="grid grid-cols-1 gap-2 py-2 @xl:grid-cols-3 sm:items-end">
							<div>
								<span class="text-slate-500">{t('phones.account')} {a.account_index}:</span>
								<a class="font-mono hover:underline" href="/extensions/{a.extension_id}"
									>{a.extension_number}</a
								>
								{a.display_name}
								{#if can('phones.manage')}
									<button
										class="btn btn-sm ml-1"
										title={t('phones.unassign')}
										onclick={() => unassign(a.device_id, a.extension_number)}>✕</button
									>
								{/if}
							</div>
							{#if texts[a.device_id]}
								<div>
									<label for="lbl-{a.device_id}">{t('phones.accountLabel')}</label>
									<input
										id="lbl-{a.device_id}"
										class="input"
										maxlength="32"
										placeholder={a.extension_number}
										disabled={!can('phones.manage')}
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
										disabled={!can('phones.manage')}
										bind:value={texts[a.device_id].phone_display_name}
									/>
								</div>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}
			{#if can('phones.manage')}
				{#if slotsFree}
					<div
						class="flex flex-wrap items-end gap-2 border-t border-slate-100 pt-3 dark:border-slate-800"
					>
						<div class="min-w-48 flex-1">
							<label for="ph-assign">{t('phones.assignExtension')}</label>
							<select id="ph-assign" class="input" bind:value={assignId}>
								<option value="">—</option>
								{#each assignable as e (e.id)}
									<option value={e.id}>{e.number} {e.display_name}</option>
								{/each}
							</select>
						</div>
						<button class="btn btn-primary" disabled={!assignId} onclick={assign}
							>{t('phones.assign')}</button
						>
					</div>
					<p class="hint">{t('phones.assignHint')}</p>
				{:else if model}
					<p class="hint">{t('phones.allAccountsUsed', { n: model.accounts })}</p>
				{/if}
			{/if}
		</section>

		<section class="card space-y-3">
			<h2>{t('phones.personalize')}</h2>
			<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
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

		{#if comfortShown && comfort && model}
			<section class="card space-y-3">
				<h2>{t('phoneset.title')}</h2>
				<p class="hint">{t('phoneset.phoneHint')}</p>
				<PhoneSettingsEditor
					catalog={comfort.catalog.filter(
						(s) =>
							(!s.vendor || s.vendor === model.vendor) &&
							(!s.models?.length || s.models.includes(model.id))
					)}
					bind:values={form.settings}
					family={model.family}
					inherited={comfort.values}
				/>
			</section>
		{/if}

		{#if model}<PhoneNativeKeys
				{model}
				bind:values={provisioning}
				disabled={!can('phones.manage')}
			/><PhoneVendorSettings
				{model}
				bind:values={provisioning}
				disabled={!can('phones.manage')}
			/><PhoneProvisioningEditor
				{model}
				accounts={phone.accounts}
				{contacts}
				{sections}
				sectionIds={form.phonebook_sections}
				bind:values={provisioning}
				disabled={!can('phones.manage') || saving}
			/>{/if}
		{#if model && model.line_keys > 0}
			{#if model.layout}
				<PhoneKeyEditor
					{model}
					accounts={phone.accounts}
					{extensions}
					{contacts}
					{sections}
					sectionIds={form.phonebook_sections}
					bind:keys
					disabled={!can('phones.manage') || saving}
				/>
			{:else}
				<section class="card space-y-3">
					<h2>{t('phones.keys')}</h2>
					<p class="hint">{t('phones.keysHint')}</p>
					<div class="scroll-x">
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
		{/if}

		{#if can('phones.manage')}
			<div class="flex flex-wrap justify-end gap-2">
				<button class="btn" disabled={saving} onclick={() => save(false)}>{t('common.save')}</button
				>
				{#if model?.provisioning_capabilities?.resync !== false}<button
						class="btn btn-primary"
						disabled={saving}
						onclick={() => save(true)}
						>{saving ? t('common.loading') : t('phonekeys.saveResync')}</button
					>{/if}
				>
			</div>
		{/if}
	{/if}
</div>

<Modal title={t('phones.showConfig')} bind:open={configOpen}>
	{#if qrImage}<img src={qrImage} alt={t('vendor.showQr')} class="mx-auto h-64 w-64" />{:else}<pre
			class="overflow-x-auto rounded-lg bg-slate-100 p-2 text-xs dark:bg-slate-800">{config}</pre>{/if}
</Modal>

<PhoneCloneDialog
	bind:open={cloneOpen}
	bind:sourceId={cloneSource}
	phones={phone ? [phone] : []}
	{models}
	{extensions}
/>

<Modal bind:open={previewOpen} title={t('prov.preview')}
	><p class="hint">{t('prov.previewHint')}</p>
	<h3>{t('prov.diff')}</h3>
	<pre class="max-h-64 overflow-auto whitespace-pre-wrap break-all text-xs">{previewDiff ||
			t('prov.noDiff')}</pre>
	<h3>{t('prov.result')}</h3>
	<pre
		class="max-h-96 overflow-auto whitespace-pre-wrap break-all text-xs">{previewText}</pre></Modal
>
