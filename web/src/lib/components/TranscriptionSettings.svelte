<script lang="ts">
	import { onMount } from 'svelte';
	import {
		api,
		type CoralHealth,
		type LocalModel,
		type TranscriptionOverview,
		type TranscriptionProfile,
		type TranscriptionProvider,
		type TranscriptionPurpose,
		type TranscriptionTest
	} from '#lib/api.ts';
	import SpeechModels from '#lib/components/SpeechModels.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { i18n, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	/** Presets for new connections; models are suggestions, any may be typed. */
	const presets = [
		{ id: 'openai', kind: 'openai', name: 'OpenAI', url: 'https://api.openai.com/v1' },
		{ id: 'groq', kind: 'compatible', name: 'Groq', url: 'https://api.groq.com/openai/v1' },
		{ id: 'mistral', kind: 'compatible', name: 'Mistral', url: 'https://api.mistral.ai/v1' },
		{ id: 'custom', kind: 'compatible', name: '', url: 'http://192.168.1.10:8000/v1' },
		// coral-stt (ADR 0046) and cuda-stt (ADR 0053) on this server: loopback
		// only, no costs.
		{ id: 'coral', kind: 'coral', name: 'Coral', url: 'http://127.0.0.1:8095/v1' },
		{ id: 'cuda', kind: 'cuda', name: 'GPU', url: 'http://127.0.0.1:8096/v1' }
	] as const;
	type PresetId = (typeof presets)[number]['id'];

	const fileModels: Record<string, string[]> = {
		'https://api.openai.com/v1': [
			'gpt-transcribe',
			'gpt-4o-mini-transcribe',
			'gpt-4o-transcribe',
			'whisper-1'
		],
		'https://api.groq.com/openai/v1': ['whisper-large-v3-turbo', 'whisper-large-v3'],
		'https://api.mistral.ai/v1': ['voxtral-mini-latest']
	};
	const realtimeModels = ['gpt-live-transcribe', 'gpt-realtime-whisper', 'gpt-4o-transcribe'];
	/** Models of the local services: language detection per turn, or one language. */
	const serviceModels: Record<'coral' | 'cuda', string[]> = {
		coral: ['coral-auto', 'coral-ctc-de', 'coral-ctc-en'],
		cuda: ['cuda-auto', 'cuda-de', 'cuda-en']
	};
	const localModels: Record<'live' | 'other', LocalModel[]> = {
		live: ['base', 'fast'],
		other: ['fast', 'accurate', 'best', 'german']
	};
	const purposes: TranscriptionPurpose[] = [
		'live',
		'recording',
		'recording_refine',
		'voicemail',
		'voicemail_refine'
	];

	let data = $state<TranscriptionOverview | null>(null);
	let profiles = $state<TranscriptionProfile[]>([]);
	let error = $state('');
	let info = $state('');
	let busy = $state(false);

	// Connection form: null = closed, '' = new, else the edited id.
	let editing = $state<string | null>(null);
	let form = $state({ preset: 'openai' as PresetId, name: 'OpenAI', url: '', key: '' });

	// Coral and CUDA run on this server: no privacy note for them.
	const anyCloud = $derived(profiles.some((p) => p.engine === 'cloud' && !isLocal(p)));
	const liveCloud = $derived(
		profiles.some((p) => p.purpose === 'live' && p.engine === 'cloud' && !isLocal(p))
	);
	const localProviders = $derived(
		data?.providers.filter((p) => p.kind === 'coral' || p.kind === 'cuda') ?? []
	);
	/** The service for "Apply hybrid": the GPU first, it runs larger models. */
	const hybridProvider = $derived(
		localProviders.find((p) => p.kind === 'cuda') ?? localProviders[0]
	);
	/** Device status of the last Coral or CUDA connection test, by connection id. */
	let coralStatus = $state<Record<string, CoralHealth>>({});

	onMount(load);

	async function load() {
		try {
			data = await api.get<TranscriptionOverview>('/settings/transcription');
			profiles = data.profiles.map((p) => ({ ...p }));
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function provider(id: string | null): TranscriptionProvider | undefined {
		return data?.providers.find((p) => p.id === id);
	}

	/** `coral` or `cuda` for a profile on a speech service of this server. */
	function localKind(p: TranscriptionProfile): 'coral' | 'cuda' | null {
		const kind = provider(p.provider_id)?.kind;
		return kind === 'coral' || kind === 'cuda' ? kind : null;
	}

	function isLocal(p: TranscriptionProfile): boolean {
		return localKind(p) !== null;
	}

	/** `Coral` or `CUDA`, for the texts of a profile or connection. */
	function serviceName(kind: string | null | undefined): string {
		return kind === 'cuda' ? 'CUDA' : 'Coral';
	}

	function suggestions(p: TranscriptionProfile): string[] {
		const kind = localKind(p);
		if (kind) return serviceModels[kind];
		if (p.realtime) return realtimeModels;
		return fileModels[provider(p.provider_id)?.url ?? ''] ?? [];
	}

	function listPrice(p: TranscriptionProfile): number | null {
		if (provider(p.provider_id)?.kind !== 'openai') return null;
		return data?.openai_prices.find((m) => m.model === p.model.trim())?.price_per_minute ?? null;
	}

	function money(value: number, digits = 4) {
		return new Intl.NumberFormat(i18n.locale === 'de' ? 'de-DE' : 'en-US', {
			style: 'currency',
			currency: 'USD',
			maximumFractionDigits: digits
		}).format(value);
	}

	function minutes(seconds: number) {
		return new Intl.NumberFormat(i18n.locale === 'de' ? 'de-DE' : 'en-US', {
			maximumFractionDigits: 1
		}).format(seconds / 60);
	}

	function setEngine(p: TranscriptionProfile, engine: TranscriptionProfile['engine']) {
		p.engine = engine;
		if (engine === 'local') {
			const allowed = localModels[p.purpose === 'live' ? 'live' : 'other'];
			if (!allowed.includes(p.local_model)) p.local_model = allowed[0];
		}
		if (engine === 'cloud' && !p.provider_id) {
			p.provider_id = data?.providers[0]?.id ?? null;
		}
		if (engine === 'cloud' && !p.model) {
			const kind = localKind(p);
			p.model = kind ? `${kind}-auto` : 'gpt-transcribe';
		}
		if (engine !== 'cloud') {
			p.realtime = false;
			p.coral_stable_preview = false;
		}
	}

	function setProvider(p: TranscriptionProfile, id: string) {
		const before = isLocal(p);
		p.provider_id = id;
		const kind = localKind(p);
		const after = kind !== null;
		if (kind && !serviceModels[kind].includes(p.model)) p.model = `${kind}-auto`;
		if (!after && before) {
			p.coral_stable_preview = false;
			p.model = 'gpt-transcribe';
			p.realtime = false;
		}
		if (after) p.price_per_minute = null;
	}

	function setRealtime(p: TranscriptionProfile, on: boolean) {
		p.realtime = on;
		if (!on) p.coral_stable_preview = false;
		if (isLocal(p)) return;
		if (on && !realtimeModels.includes(p.model)) p.model = realtimeModels[0];
		if (!on && realtimeModels.slice(0, 2).includes(p.model)) p.model = 'gpt-transcribe';
	}

	/** Coral or CUDA for the live preview (Realtime) and the first passes; local Whisper refines. */
	function applyHybrid() {
		const local = hybridProvider;
		if (!local) return;
		for (const p of profiles) {
			if (p.purpose.endsWith('_refine')) {
				const keep = p.engine === 'local' && localModels.other.includes(p.local_model);
				p.engine = 'local';
				p.local_model = keep ? p.local_model : 'fast';
				p.realtime = false;
				p.coral_stable_preview = false;
				continue;
			}
			p.engine = 'cloud';
			p.provider_id = local.id;
			p.model = `${local.kind}-auto`;
			p.price_per_minute = null;
			p.realtime = p.purpose === 'live';
			// The local fallback: base for the live preview, else the current or fast.
			if (p.purpose === 'live') p.local_model = 'base';
			else if (!localModels.other.includes(p.local_model)) p.local_model = 'fast';
		}
		info = t('stt.hybridApplied');
		error = '';
	}

	async function saveProfiles() {
		error = '';
		info = '';
		try {
			const body = profiles.map((p) => ({
				...p,
				model: p.model.trim(),
				price_per_minute:
					p.price_per_minute === null || (p.price_per_minute as unknown) === ''
						? null
						: Number(p.price_per_minute)
			}));
			const saved = await api.put<TranscriptionProfile[]>('/settings/transcription/profiles', body);
			profiles = saved.map((p) => ({ ...p }));
			if (data) data.profiles = saved;
			info = t('common.saved');
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function openForm(p?: TranscriptionProvider) {
		error = '';
		info = '';
		if (p) {
			const preset =
				p.kind === 'coral' || p.kind === 'cuda'
					? p.kind
					: (presets.find((x) => x.id !== 'custom' && x.kind === p.kind && x.url === p.url)?.id ??
						'custom');
			form = { preset, name: p.name, url: p.url, key: '' };
			editing = p.id;
		} else {
			form = { preset: 'openai', name: 'OpenAI', url: presets[0].url, key: '' };
			editing = '';
		}
	}

	function pickPreset(id: PresetId) {
		const p = presets.find((x) => x.id === id)!;
		form.preset = id;
		form.url = id === 'custom' ? '' : p.url;
		if (!form.name || presets.some((x) => x.name === form.name)) form.name = p.name;
	}

	async function saveProvider() {
		error = '';
		info = '';
		const preset = presets.find((x) => x.id === form.preset)!;
		const body = {
			name: form.name.trim(),
			kind: preset.kind,
			url: form.url.trim(),
			key: form.key || null
		};
		if ((preset.kind === 'coral' || preset.kind === 'cuda') && !body.url) body.url = preset.url;
		try {
			if (editing) {
				await api.put<TranscriptionProvider>(`/settings/transcription/providers/${editing}`, body);
			} else {
				await api.post<TranscriptionProvider>('/settings/transcription/providers', body);
			}
			editing = null;
			await load();
			info = t('common.saved');
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function removeKey(p: TranscriptionProvider) {
		error = '';
		try {
			await api.put(`/settings/transcription/providers/${p.id}`, {
				name: p.name,
				kind: p.kind,
				url: p.url,
				key: ''
			});
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function removeProvider(p: TranscriptionProvider) {
		if (!confirm(t('stt.deleteConfirm', { name: p.name }))) return;
		error = '';
		try {
			await api.del(`/settings/transcription/providers/${p.id}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function test(p: TranscriptionProvider) {
		busy = true;
		error = '';
		info = '';
		try {
			const r = await api.post<TranscriptionTest>(`/settings/transcription/providers/${p.id}/test`);
			if (r.coral) coralStatus = { ...coralStatus, [p.id]: r.coral };
			info =
				r.missing_models === null
					? t('stt.testReachable')
					: r.missing_models.length
						? t('stt.testMissing', {
								models: r.missing_models.join(', '),
								available: r.speech_models.join(', ') || '–'
							})
						: t('stt.testFine', { models: r.speech_models.join(', ') || '–' });
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<!-- Inside the settings form, so no <form> of its own. -->
<div class="space-y-4" data-testid="transcription-settings">
	<ErrorBox {error} />
	{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400" data-testid="stt-info">
			{info}
		</p>{/if}

	<section class="space-y-3 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
		<h3 class="font-medium">{t('stt.profilesTitle')}</h3>
		<p class="hint">{t('stt.profilesHint')}</p>
		{#if anyCloud}
			<p
				class="rounded-md bg-amber-50 p-2 text-sm text-amber-900 dark:bg-amber-950/40 dark:text-amber-200"
			>
				{t('stt.privacy')}
				{#if liveCloud}{t('stt.privacyLive')}{/if}
			</p>
		{/if}
		{#if data}
			{#each purposes as purpose (purpose)}
				{@const p = profiles.find((x) => x.purpose === purpose)!}
				{@const id = `stt-${purpose}`}
				<fieldset
					class="space-y-2 border-t border-slate-200 pt-3 first-of-type:border-t-0 first-of-type:pt-0 dark:border-slate-700"
					data-testid={id}
				>
					<legend class="sr-only">{t(`stt.purpose.${purpose}`)}</legend>
					<div
						class="grid grid-cols-1 gap-2 @xl:grid-cols-[minmax(0,14rem)_minmax(0,1fr)] @xl:items-start"
					>
						<div>
							<label for={`${id}-engine`} class="font-medium">{t(`stt.purpose.${purpose}`)}</label>
							<p class="hint">{t(`stt.purposeHint.${purpose}`)}</p>
						</div>
						<div class="min-w-0 space-y-2">
							<select
								id={`${id}-engine`}
								class="input"
								value={p.engine}
								onchange={(e) =>
									setEngine(
										p,
										(e.currentTarget as HTMLSelectElement).value as TranscriptionProfile['engine']
									)}
							>
								{#if purpose.endsWith('_refine')}<option value="off">{t('stt.engineOff')}</option
									>{/if}
								<option value="local">{t('stt.engineLocal')}</option>
								<option value="cloud" disabled={!data.providers.length}
									>{localProviders.length ? t('stt.engineConnection') : t('stt.engineCloud')}{data
										.providers.length
										? ''
										: ` – ${t('stt.noProvider')}`}</option
								>
							</select>
							{#if p.engine === 'local'}
								<select class="input" aria-label={t('stt.localModel')} bind:value={p.local_model}>
									{#each localModels[purpose === 'live' ? 'live' : 'other'] as m (m)}
										<option value={m}>{t(`stt.local.${m}`)}</option>
									{/each}
								</select>
							{:else if p.engine === 'cloud'}
								<div class="grid grid-cols-1 gap-2 @lg:grid-cols-2">
									<select
										class="input"
										aria-label={t('stt.provider')}
										value={p.provider_id}
										onchange={(e) => setProvider(p, (e.currentTarget as HTMLSelectElement).value)}
									>
										{#each data.providers as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
									</select>
									<input
										class="input font-mono"
										aria-label={t('stt.model')}
										list={`${id}-models`}
										bind:value={p.model}
										required
									/>
									<datalist id={`${id}-models`}>
										{#each suggestions(p) as m (m)}<option value={m}></option>{/each}
									</datalist>
								</div>
								{#if purpose === 'live'}
									<label class="flex items-start gap-2 text-sm"
										><input
											type="checkbox"
											class="mt-1"
											checked={p.realtime}
											onchange={(e) =>
												setRealtime(p, (e.currentTarget as HTMLInputElement).checked)}
										/>
										<span
											>{isLocal(p)
												? t('stt.coralRealtime', { service: serviceName(localKind(p)) })
												: t('stt.realtime')}<br /><span class="hint"
												>{isLocal(p)
													? t('stt.coralRealtimeHint', { service: serviceName(localKind(p)) })
													: t('stt.realtimeHint')}</span
											></span
										></label
									>
								{/if}
								{#if isLocal(p)}
									{#if purpose === 'live' && p.realtime}
										<label class="flex items-start gap-2 text-sm">
											<input
												type="checkbox"
												class="mt-1"
												bind:checked={p.coral_stable_preview}
												data-testid="stt-coral-stable-preview"
											/>
											<span
												>{t('stt.coralStablePreview')}<br /><span class="hint"
													>{t('stt.coralStablePreviewHint')}</span
												></span
											>
										</label>
									{/if}
									{#if purpose === 'live'}
										<p class="hint" data-testid={`${id}-fallback`}>
											{t('stt.fallbackLive', { service: serviceName(localKind(p)) })}
										</p>
									{:else}
										<div>
											<label for={`${id}-fallback`} class="text-sm">{t('stt.fallback')}</label>
											<select
												id={`${id}-fallback`}
												class="input"
												bind:value={p.local_model}
												data-testid={`${id}-fallback`}
											>
												{#each localModels.other as m (m)}
													<option value={m}>{t(`stt.local.${m}`)}</option>
												{/each}
											</select>
											<p class="hint">
												{t('stt.fallbackHint', { service: serviceName(localKind(p)) })}
											</p>
										</div>
									{/if}
								{:else}
									<div class="flex flex-wrap items-center gap-2 text-sm">
										<label for={`${id}-price`}>{t('stt.price')}</label>
										<input
											id={`${id}-price`}
											class="input w-28"
											type="number"
											min="0"
											max="10"
											step="0.0001"
											bind:value={p.price_per_minute}
											placeholder={listPrice(p)?.toString() ?? ''}
										/>
										<span class="hint"
											>{listPrice(p) !== null
												? t('stt.listPrice', { price: money(listPrice(p)!) })
												: t('stt.noListPrice')}</span
										>
									</div>
								{/if}
							{/if}
						</div>
					</div>
				</fieldset>
			{/each}
			<p class="hint">{t('rec.qualityHint')}</p>
			{#if hybridProvider}<p class="hint">
					{t('stt.hybridHint', { service: serviceName(hybridProvider.kind) })}
				</p>{/if}
			<div class="flex flex-wrap justify-end gap-2">
				{#if hybridProvider}
					<button type="button" class="btn" onclick={applyHybrid} data-testid="stt-hybrid"
						>{t('stt.hybrid')}</button
					>
				{/if}
				<button type="button" class="btn btn-primary" onclick={saveProfiles}
					>{t('stt.saveProfiles')}</button
				>
			</div>
		{/if}
	</section>

	<section class="space-y-3 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
		<h3 class="font-medium">{t('stt.providersTitle')}</h3>
		<p class="hint">{t('stt.providersHint')}</p>
		{#if data}
			<ul class="divide-y divide-slate-200 dark:divide-slate-700">
				{#each data.providers as c (c.id)}
					<li class="flex flex-wrap items-center gap-2 py-2" data-testid="stt-provider">
						<div class="min-w-0 basis-full sm:flex-1 sm:basis-auto">
							<p class="font-medium">
								{c.name}{#if c.kind === 'coral'}<span class="badge ml-2">{t('stt.coralBadge')}</span
									>{:else if c.kind === 'cuda'}<span class="badge ml-2">{t('stt.cudaBadge')}</span
									>{/if}
							</p>
							<p class="truncate font-mono text-xs text-slate-500 dark:text-slate-400">
								{c.url} · {c.has_key ? t('stt.keyStoredShort') : t('stt.keyNone')}
							</p>
						</div>
						<button type="button" class="btn" disabled={busy} onclick={() => test(c)}
							>{t('stt.test')}</button
						>
						<button type="button" class="btn" onclick={() => openForm(c)}>{t('common.edit')}</button
						>
						{#if c.has_key}
							<button type="button" class="btn" onclick={() => removeKey(c)}
								>{t('stt.keyRemove')}</button
							>
						{/if}
						<button type="button" class="btn" onclick={() => removeProvider(c)}
							>{t('common.delete')}</button
						>
						{#if coralStatus[c.id]}
							{@const h = coralStatus[c.id]}
							<div
								class="basis-full rounded-md bg-slate-50 p-2 text-sm dark:bg-slate-800/50"
								data-testid="stt-coral-status"
							>
								<p class="font-medium">
									{t('stt.coralStatus', { service: serviceName(c.kind) })}:
									<span
										class="badge {h.status === 'ok'
											? 'badge-ok'
											: h.status === 'degraded'
												? 'badge-warn'
												: 'badge-bad'}">{t(`stt.coralState.${h.status}`)}</span
									>
								</p>
								{#if c.kind === 'cuda'}
									{#if h.gpu}
										<p class="break-words">
											{t('stt.gpuStatus', {
												name: h.gpu.name ?? 'GPU',
												used: h.gpu.memory_used_mib ?? '–',
												total: h.gpu.memory_total_mib ?? '–',
												util: h.gpu.utilization_percent ?? '–',
												temp: h.gpu.temperature_c ?? '–'
											})}
										</p>
									{:else}
										<p class="hint">{t('stt.gpuNone')}</p>
									{/if}
									<ul class="mt-1 space-y-0.5">
										{#each h.device?.models ?? [] as m (m.slot)}
											<li class="break-words">
												{t('stt.gpuModel', {
													slot: m.slot,
													name: m.name,
													state: m.error ? `${m.state} (${m.error})` : m.state,
													vram: m.vram_mib ?? '–'
												})}
											</li>
										{/each}
									</ul>
								{:else if h.tpus?.length}
									<ul class="mt-1 space-y-0.5">
										{#each h.tpus as tpu (tpu.id)}
											<li class="break-words">
												{t('stt.coralTpu', {
													id: tpu.id,
													kind: tpu.kind.toUpperCase(),
													state: tpu.error ? `${tpu.state} (${tpu.error})` : tpu.state,
													load: Math.round(tpu.load * 100),
													models: tpu.models.join(', ') || '–'
												})}
											</li>
										{/each}
									</ul>
								{:else}
									<p class="hint">{t('stt.coralNoTpu')}</p>
								{/if}
								<p class="hint">
									{t('stt.coralQueue', {
										length: h.queue.length,
										capacity: h.queue.capacity,
										sessions: h.queue.sessions,
										max: h.queue.max_sessions
									})}
								</p>
							</div>
						{/if}
					</li>
				{:else}
					<li class="py-2 text-sm text-slate-500 dark:text-slate-400">{t('stt.noProvider')}</li>
				{/each}
			</ul>
			{#if editing === null}
				<div class="flex justify-end">
					<button type="button" class="btn" onclick={() => openForm()}
						>{t('stt.addProvider')}</button
					>
				</div>
			{:else}
				<div class="space-y-3 rounded-md bg-slate-50 p-3 dark:bg-slate-800/50">
					<div>
						<label for="stt-preset">{t('stt.providerType')}</label>
						<select
							id="stt-preset"
							class="input"
							value={form.preset}
							onchange={(e) => pickPreset((e.currentTarget as HTMLSelectElement).value as PresetId)}
						>
							{#each presets as p (p.id)}
								<option value={p.id}>{t(`stt.provider.${p.id}`)}</option>
							{/each}
						</select>
					</div>
					<div>
						<label for="stt-name">{t('stt.name')}</label>
						<input id="stt-name" class="input" maxlength="60" bind:value={form.name} required />
					</div>
					{#if form.preset === 'coral'}<p class="hint">{t('stt.coralHint')}</p>{/if}
					{#if form.preset === 'cuda'}<p class="hint">{t('stt.cudaHint')}</p>{/if}
					{#if form.preset !== 'openai'}
						<div>
							<label for="stt-url">{t('stt.url')}</label>
							<input
								id="stt-url"
								class="input font-mono"
								bind:value={form.url}
								placeholder="https://…/v1"
							/>
							<p class="hint">{t('stt.urlHint')}</p>
						</div>
					{/if}
					<div>
						<label for="stt-key">{t('stt.key')}</label>
						<input
							id="stt-key"
							class="input font-mono"
							type="password"
							autocomplete="off"
							bind:value={form.key}
							placeholder={editing && provider(editing)?.has_key
								? t('stt.keyStored')
								: t('stt.keyNone')}
						/>
					</div>
					<div class="flex justify-end gap-2">
						<button type="button" class="btn" onclick={() => (editing = null)}
							>{t('common.cancel')}</button
						>
						<button type="button" class="btn btn-primary" onclick={saveProvider}
							>{t('stt.saveProvider')}</button
						>
					</div>
				</div>
			{/if}
		{/if}
	</section>

	{#each localProviders as c (c.id)}<SpeechModels provider={c} />{/each}

	<section class="space-y-2 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
		<h3 class="font-medium">{t('stt.usageTitle')}</h3>
		<p class="hint">{t('stt.usageHint')}</p>
		{#if data?.usage.length}
			<div class="scroll-x">
				<table class="w-full text-sm" data-testid="stt-usage">
					<thead class="text-left text-slate-500 dark:text-slate-400">
						<tr>
							<th class="py-1 pr-2 font-normal">{t('stt.usageMonth')}</th>
							<th class="py-1 pr-2 font-normal">{t('stt.usagePurpose')}</th>
							<th class="py-1 pr-2 font-normal">{t('stt.model')}</th>
							<th class="py-1 pr-2 text-right font-normal">{t('stt.usageMinutes')}</th>
							<th class="py-1 text-right font-normal">{t('stt.usageCost')}</th>
						</tr>
					</thead>
					<tbody>
						{#each data.usage as u (`${u.month}:${u.purpose}:${u.engine}`)}
							<tr class="border-t border-slate-200 dark:border-slate-700">
								<td class="py-1 pr-2">{u.month.slice(0, 7)}</td>
								<td class="py-1 pr-2">{t(`stt.purpose.${u.purpose}`)}</td>
								<td class="py-1 pr-2 font-mono text-xs break-all">{u.engine}</td>
								<td class="py-1 pr-2 text-right">{minutes(u.seconds)}</td>
								<td class="py-1 text-right"
									>{money(u.cost, 2)}{#if u.unpriced_seconds > 0}*{/if}</td
								>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
			{#if data.usage.some((u) => u.unpriced_seconds > 0)}
				<p class="hint">{t('stt.usageUnpriced')}</p>
			{/if}
		{:else}
			<p class="text-sm text-slate-500 dark:text-slate-400">{t('stt.usageNone')}</p>
		{/if}
	</section>
</div>
