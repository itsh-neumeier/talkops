<script lang="ts">
	import {
		api,
		upload,
		type CoralLibraryModel,
		type CoralModels,
		type CoralSlot,
		type CudaFamily,
		type CudaLibraryModel,
		type CudaModels,
		type CudaPreset,
		type TranscriptionProvider
	} from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	/**
	 * Model library of a speech service on this server: coral-stt (ADR 0048)
	 * or cuda-stt (ADR 0049). The service stores and checks the models.
	 */
	let { provider }: { provider: TranscriptionProvider } = $props();

	const slots: CoralSlot[] = ['de', 'en', 'langid'];
	const cuda = $derived(provider.kind === 'cuda');
	const service = $derived(cuda ? 'CUDA' : 'Coral');
	const base = $derived(`/speech-services/${provider.id}`);

	/** One library model, the same for both services. */
	interface Row {
		id: string;
		name: string;
		detail: string;
		license: string;
		source: string;
		installed: number;
		bytes: number;
		inUse: boolean;
		/** Slots this model can fill. */
		fits: CoralSlot[];
	}
	/** One slot, the same for both services. */
	interface SlotRow {
		slot: CoralSlot;
		modelId: string;
		active: boolean;
		selected: boolean;
		error: string | null;
		vram: number | null;
	}

	let coralData = $state<CoralModels | null>(null);
	let cudaData = $state<CudaModels | null>(null);
	let error = $state('');
	let info = $state('');
	let busy = $state(false);
	// Install form: inside the settings form, so no <form> of its own.
	let slot = $state<CoralSlot>('de');
	let name = $state('');
	let license = $state('');
	let source = $state('');
	let allowed = $state(false);
	let selectNow = $state(true);
	let metaFile = $state<FileList>();
	let tpuFile = $state<FileList>();
	let cpuFile = $state<FileList>();
	let archiveFile = $state<FileList>();
	let family = $state<CudaFamily>('nemo_transducer');
	let languages = $state('de');
	let precision = $state<'full' | 'int8'>('full');
	let url = $state('');
	let sha256 = $state('');

	const rows = $derived<Row[]>(
		cuda
			? (cudaData?.library ?? []).map((m: CudaLibraryModel) => ({
					id: m.id,
					name: m.manifest.name,
					detail: `${m.manifest.family} · ${m.manifest.languages.join(', ')}`,
					license: m.manifest.license,
					source: m.manifest.source,
					installed: m.manifest.installed_at_unix,
					bytes: m.bytes,
					inUse: m.slots.length > 0,
					fits: [
						...(['de', 'en'] as CoralSlot[]).filter((s) => m.manifest.languages.includes(s)),
						...(m.manifest.family === 'whisper' ? (['langid'] as CoralSlot[]) : [])
					]
				}))
			: (coralData?.library ?? []).map((m: CoralLibraryModel) => ({
					id: m.id,
					name: m.info.name,
					detail: m.candidate ?? '',
					license: m.info.license,
					source: m.info.source,
					installed: m.info.installed_at_unix,
					bytes: m.bytes,
					inUse: m.selected,
					fits: [m.info.slot]
				}))
	);
	const slotRows = $derived<SlotRow[]>(
		cuda
			? (cudaData?.slots ?? []).map((s) => ({
					slot: s.slot,
					modelId: s.model_id ?? '',
					active: s.active,
					selected: !!s.model_id,
					error: s.error,
					vram: s.vram_mib
				}))
			: (coralData?.slots ?? []).map((s) => ({
					slot: s.slot,
					modelId: s.installed ? (s.model_id ?? '') : '',
					active: s.active,
					selected: s.installed,
					error: null,
					vram: null
				}))
	);
	const usage = $derived(cuda ? cudaData?.usage : coralData?.usage);
	const backend = $derived(cuda ? cudaData?.backend : coralData?.backend);
	const loaded = $derived(cuda ? !!cudaData : !!coralData);
	const pending = $derived(
		cuda
			? (cudaData?.slots.some((s) => (!!s.model_id && !s.active) || (!s.model_id && s.loaded)) ??
					false)
			: (coralData?.slots.some((s) => (s.installed && !s.active) || (!s.installed && s.loaded)) ??
					false)
	);
	const full = $derived(!!usage && usage.models >= usage.max_models);
	const statementsOk = $derived(
		allowed && !!name.trim() && !!license.trim() && !!source.trim() && !busy && !full
	);

	function mib(bytes: number): string {
		return (bytes / 1024 / 1024).toFixed(1);
	}
	function slotName(s: CoralSlot): string {
		return t(`speechModels.slot.${s}`);
	}

	async function refresh() {
		error = '';
		try {
			if (cuda) cudaData = await api.get<CudaModels>(`${base}/models`);
			else coralData = await api.get<CoralModels>(`${base}/models`);
		} catch (err) {
			error = errorMessage(err);
		}
	}
	$effect(() => {
		if (provider.id) void refresh();
	});

	async function run(action: () => Promise<unknown>, done: string) {
		busy = true;
		error = '';
		info = '';
		try {
			await action();
			info = done;
			await refresh();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	function clearForm() {
		name = license = source = url = sha256 = '';
		allowed = false;
		metaFile = tpuFile = cpuFile = archiveFile = undefined;
	}

	function done(): string {
		return selectNow ? t('speechModels.addedSelected', { service }) : t('speechModels.added');
	}

	function addCoral() {
		const meta = metaFile?.[0];
		const tpu = tpuFile?.[0];
		if (!meta || !tpu) {
			error = t('speechModels.filesRequired');
			return;
		}
		const form = new FormData();
		form.append('slot', slot);
		form.append('name', name);
		form.append('license', license);
		form.append('source', source);
		form.append('model_json', meta);
		form.append('model_edgetpu', tpu);
		const cpu = cpuFile?.[0];
		if (cpu) form.append('model_cpu', cpu);
		void run(async () => {
			const model = await upload<CoralLibraryModel>(`${base}/library`, form);
			if (selectNow) await api.put(`${base}/slots/${model.info.slot}`, { model: model.id });
			clearForm();
		}, done());
	}

	function addCuda() {
		const archive = archiveFile?.[0];
		if (!archive) {
			error = t('speechModels.archiveRequired');
			return;
		}
		const form = new FormData();
		for (const [key, value] of Object.entries({ name, family, languages, license, source })) {
			form.append(key, value);
		}
		form.append('precision', precision);
		if (sha256.trim()) form.append('sha256', sha256.trim());
		if (selectNow) form.append('slot', slot);
		form.append('archive', archive);
		void run(async () => {
			await upload<CudaLibraryModel>(`${base}/library`, form);
			clearForm();
		}, done());
	}

	function download() {
		void run(async () => {
			await api.post<CudaLibraryModel>(`${base}/library/download`, {
				url: url.trim(),
				sha256: sha256.trim(),
				name,
				family,
				languages,
				license,
				source,
				precision,
				slot: selectNow ? slot : null
			});
			clearForm();
		}, done());
	}

	/** Fills the form with a suggested model; the license still needs a confirmation. */
	function usePreset(p: CudaPreset) {
		name = p.name;
		family = p.family;
		languages = p.languages;
		license = p.license;
		source = p.source;
		url = p.url;
		sha256 = p.sha256;
		precision = p.precision;
		slot = p.slots[0];
		allowed = false;
		info = '';
	}

	function select(s: CoralSlot, id: string) {
		if (!id) {
			if (!confirm(t('speechModels.confirmClear', { slot: slotName(s) }))) {
				void refresh();
				return;
			}
			void run(() => api.del(`${base}/slots/${s}`), t('speechModels.selected', { service }));
			return;
		}
		void run(
			() => api.put(`${base}/slots/${s}`, { model: id }),
			t('speechModels.selected', { service })
		);
	}

	function remove(m: Row) {
		if (!confirm(t('common.confirmDelete', { name: m.name }))) return;
		void run(() => api.del(`${base}/library/${m.id}`), t('speechModels.deleted'));
	}

	function restart() {
		if (!confirm(t('speechModels.confirmRestart', { service }))) return;
		void run(
			async () => {
				await api.post(`${base}/restart`);
				// The service needs a few seconds to load the models again.
				await new Promise((resolve) => setTimeout(resolve, cuda ? 15000 : 8000));
			},
			t('speechModels.restarted', { service })
		);
	}
</script>

<section
	class="space-y-3 rounded-lg border border-slate-200 p-3 dark:border-slate-700"
	data-testid="speech-models-{provider.kind}"
>
	<h3 class="font-medium">
		{t('speechModels.title', { service })}
		<span class="hint font-normal">· {provider.name}</span>
	</h3>
	<p class="hint">{t('speechModels.hint')}</p>
	<ErrorBox {error} />
	{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400" role="status">{info}</p>{/if}

	{#if loaded}
		<div class="grid gap-3 sm:grid-cols-3">
			{#each slotRows as s (s.slot)}
				<div>
					<label for="{provider.id}-slot-{s.slot}">{slotName(s.slot)}</label>
					<select
						id="{provider.id}-slot-{s.slot}"
						class="input"
						value={s.modelId}
						onchange={(e) => select(s.slot, e.currentTarget.value)}
						disabled={busy}
						data-testid="speech-slot-{s.slot}"
					>
						<option value="">{t('speechModels.none')}</option>
						{#each rows.filter((m) => m.fits.includes(s.slot)) as m (m.id)}<option value={m.id}
								>{m.name}</option
							>{/each}
					</select>
					<p class="mt-1">
						{#if s.active}<span class="badge badge-ok">{t('speechModels.active')}</span
							>{#if s.vram !== null}<span class="hint">
									· {s.vram} MiB {t('speechModels.vram')}</span
								>{/if}
						{:else if s.selected}<span class="badge badge-warn">{t('speechModels.pending')}</span>
						{:else}<span class="badge">{t('speechModels.empty')}</span>{/if}
					</p>
					{#if s.error}<p class="hint break-words text-amber-700 dark:text-amber-400">
							{t('speechModels.loadError', { error: s.error })}
						</p>{/if}
				</div>
			{/each}
		</div>
		{#if backend === 'mock'}<p class="hint">{t('speechModels.mockHint')}</p>{/if}
		{#if pending}
			<div class="flex flex-wrap items-center gap-3">
				<p class="hint">{t('speechModels.restartHint', { service })}</p>
				<button
					type="button"
					class="btn btn-primary"
					onclick={restart}
					disabled={busy}
					data-testid="speech-models-restart">{t('speechModels.restart', { service })}</button
				>
			</div>
		{/if}

		<h4 class="text-sm font-medium">{t('speechModels.libraryTitle')}</h4>
		{#if usage}
			<p class="hint">
				{t('speechModels.usage', {
					models: usage.models,
					max: usage.max_models,
					mib: mib(usage.bytes),
					maxMib: mib(usage.max_bytes)
				})}
			</p>
		{/if}
		{#if rows.length}
			<div class="scroll-x">
				<table class="w-full text-sm" data-testid="speech-library">
					<thead class="text-left text-slate-500 dark:text-slate-400">
						<tr>
							<th class="py-1 pr-2 font-normal">{t('speechModels.model')}</th>
							<th class="py-1 pr-2 font-normal">{t('speechModels.slot')}</th>
							<th class="py-1 pr-2 font-normal">{t('speechModels.license')}</th>
							<th class="py-1 pr-2 text-right font-normal">MiB</th>
							<th class="py-1 font-normal"></th>
						</tr>
					</thead>
					<tbody>
						{#each rows as m (m.id)}
							<tr class="border-t border-slate-200 align-top dark:border-slate-700">
								<td class="py-1 pr-2">
									{m.name}
									{#if m.inUse}<span class="badge badge-ok ml-1">{t('speechModels.inUse')}</span
										>{/if}
									<br /><span class="font-mono text-xs break-all text-slate-500">{m.id}</span>
									{#if m.detail}<span class="hint"> · {m.detail}</span>{/if}
									<br /><span class="hint"
										>{formatDateTime(new Date(m.installed * 1000).toISOString())}</span
									>
								</td>
								<td class="py-1 pr-2">{m.fits.map(slotName).join(', ')}</td>
								<td class="py-1 pr-2 break-words">
									{#if m.license}{m.license}{#if m.source}<br /><span class="hint break-all"
												>{m.source}</span
											>{/if}
									{:else}<span class="badge badge-warn">{t('speechModels.licenseUnknown')}</span
										>{/if}
								</td>
								<td class="py-1 pr-2 text-right">{mib(m.bytes)}</td>
								<td class="py-1 text-right">
									<button
										type="button"
										class="btn"
										onclick={() => remove(m)}
										disabled={busy || m.inUse}
										title={m.inUse ? t('speechModels.deleteInUse') : undefined}
										>{t('common.delete')}</button
									>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else}
			<p class="hint">{t('speechModels.libraryEmpty')}</p>
		{/if}

		{#if cuda && cudaData?.presets.models.length}
			<h4 class="text-sm font-medium">{t('speechModels.presetsTitle')}</h4>
			<p class="hint">{t('speechModels.presetsHint')}</p>
			<div class="scroll-x">
				<table class="w-full text-sm" data-testid="speech-presets">
					<tbody>
						{#each cudaData.presets.models as p (p.id)}
							<tr class="border-t border-slate-200 align-top dark:border-slate-700">
								<td class="py-1 pr-2"
									>{p.name}<br /><span class="hint"
										>{p.family} · {p.license} · {mib(p.bytes)} MiB</span
									></td
								>
								<td class="py-1 pr-2">{p.slots.map(slotName).join(', ')}</td>
								<td class="py-1 text-right"
									><button type="button" class="btn" onclick={() => usePreset(p)} disabled={busy}
										>{t('speechModels.usePreset')}</button
									></td
								>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
	{/if}

	<div class="space-y-3 border-t border-slate-200 pt-3 dark:border-slate-700">
		<h4 class="text-sm font-medium">{t('speechModels.uploadTitle')}</h4>
		<p class="hint">{cuda ? t('speechModels.uploadHintCuda') : t('speechModels.uploadHint')}</p>
		{#if full}<p class="hint text-amber-700 dark:text-amber-400">{t('speechModels.full')}</p>{/if}
		<div class="grid gap-3 sm:grid-cols-2">
			<div>
				<label for="{provider.id}-model-slot">{t('speechModels.slot')}</label>
				<select id="{provider.id}-model-slot" class="input" bind:value={slot} disabled={busy}>
					{#each slots as s (s)}<option value={s}>{slotName(s)}</option>{/each}
				</select>
			</div>
			<div>
				<label for="{provider.id}-model-name">{t('speechModels.name')}</label>
				<input
					id="{provider.id}-model-name"
					class="input"
					maxlength="200"
					placeholder={cuda ? 'FastConformer DE' : 'Citrinet 256 DE 2026-11'}
					bind:value={name}
					disabled={busy}
				/>
			</div>
			<div>
				<label for="{provider.id}-model-license">{t('speechModels.license')}</label>
				<input
					id="{provider.id}-model-license"
					class="input"
					maxlength="200"
					placeholder="CC-BY-4.0"
					bind:value={license}
					disabled={busy}
				/>
			</div>
			<div>
				<label for="{provider.id}-model-source">{t('speechModels.source')}</label>
				<input
					id="{provider.id}-model-source"
					class="input"
					maxlength="200"
					placeholder="https://huggingface.co/…"
					bind:value={source}
					disabled={busy}
				/>
			</div>
			{#if cuda}
				<div>
					<label for="{provider.id}-model-family">{t('speechModels.family')}</label>
					<select id="{provider.id}-model-family" class="input" bind:value={family} disabled={busy}>
						{#each cudaData?.families ?? ['nemo_ctc', 'nemo_transducer', 'canary', 'whisper'] as f (f)}<option
								value={f}>{f}</option
							>{/each}
					</select>
				</div>
				<div>
					<label for="{provider.id}-model-languages">{t('speechModels.languages')}</label>
					<input
						id="{provider.id}-model-languages"
						class="input font-mono"
						maxlength="100"
						bind:value={languages}
						disabled={busy}
					/>
				</div>
				<div>
					<label for="{provider.id}-model-precision">{t('speechModels.precision')}</label>
					<select
						id="{provider.id}-model-precision"
						class="input"
						bind:value={precision}
						disabled={busy}
					>
						<option value="full">{t('speechModels.precision.full')}</option>
						<option value="int8">{t('speechModels.precision.int8')}</option>
					</select>
				</div>
				<div>
					<label for="{provider.id}-model-sha">{t('speechModels.sha256')}</label>
					<input
						id="{provider.id}-model-sha"
						class="input font-mono"
						maxlength="64"
						bind:value={sha256}
						disabled={busy}
					/>
				</div>
			{/if}
		</div>
		<label class="flex items-start gap-2 text-sm"
			><input type="checkbox" bind:checked={selectNow} disabled={busy} /><span
				>{t('speechModels.selectNow')}</span
			></label
		>
		<label class="flex items-start gap-2 text-sm"
			><input type="checkbox" bind:checked={allowed} disabled={busy} /><span
				>{t('speechModels.confirmLicense')}</span
			></label
		>

		{#if cuda}
			<div class="space-y-2 rounded-md bg-slate-50 p-3 dark:bg-slate-800/50">
				<h5 class="text-sm font-medium">{t('speechModels.downloadTitle')}</h5>
				<p class="hint">{t('speechModels.downloadHint')}</p>
				<label for="{provider.id}-model-url">{t('speechModels.url')}</label>
				<input
					id="{provider.id}-model-url"
					class="input font-mono"
					maxlength="2048"
					placeholder="https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/…"
					bind:value={url}
					disabled={busy}
				/>
				<button
					type="button"
					class="btn btn-primary"
					onclick={download}
					disabled={!statementsOk ||
						!url.trim().startsWith('https://') ||
						sha256.trim().length !== 64}
					data-testid="speech-models-download">{t('speechModels.download')}</button
				>
			</div>
			<div class="space-y-2">
				<label for="{provider.id}-model-archive">{t('speechModels.archive')}</label>
				<input
					id="{provider.id}-model-archive"
					class="input"
					type="file"
					accept=".tar,.tgz,.gz,.bz2,application/x-tar,application/gzip,application/x-bzip2"
					bind:files={archiveFile}
					disabled={busy}
				/>
				<button
					type="button"
					class="btn btn-primary"
					onclick={addCuda}
					disabled={!statementsOk}
					data-testid="speech-models-install">{t('speechModels.install')}</button
				>
			</div>
		{:else}
			<div class="grid gap-3 sm:grid-cols-2">
				<div>
					<label for="{provider.id}-model-json">model.json</label>
					<input
						id="{provider.id}-model-json"
						class="input"
						type="file"
						accept=".json,application/json"
						bind:files={metaFile}
						disabled={busy}
					/>
				</div>
				<div>
					<label for="{provider.id}-model-tpu">model_edgetpu.tflite</label>
					<input
						id="{provider.id}-model-tpu"
						class="input"
						type="file"
						accept=".tflite"
						bind:files={tpuFile}
						disabled={busy}
					/>
				</div>
				<div>
					<label for="{provider.id}-model-cpu"
						>model.tflite <span class="hint">({t('speechModels.optional')})</span></label
					>
					<input
						id="{provider.id}-model-cpu"
						class="input"
						type="file"
						accept=".tflite"
						bind:files={cpuFile}
						disabled={busy}
					/>
				</div>
			</div>
			<button
				type="button"
				class="btn btn-primary"
				onclick={addCoral}
				disabled={!statementsOk}
				data-testid="speech-models-install">{t('speechModels.install')}</button
			>
		{/if}
	</div>
</section>
