<script lang="ts">
	import {
		api,
		upload,
		type CoralLibraryModel,
		type CoralModels,
		type CoralSlot
	} from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	/** Coral connection whose service stores the models (ADR 0048). */
	let { providerId }: { providerId: string } = $props();

	const slots: CoralSlot[] = ['de', 'en', 'langid'];
	let data = $state<CoralModels | null>(null);
	let error = $state('');
	let info = $state('');
	let busy = $state(false);
	// Upload form: inside the settings form, so no <form> of its own.
	let slot = $state<CoralSlot>('de');
	let name = $state('');
	let license = $state('');
	let source = $state('');
	let allowed = $state(false);
	let selectNow = $state(true);
	let metaFile = $state<FileList>();
	let tpuFile = $state<FileList>();
	let cpuFile = $state<FileList>();

	const pending = $derived(
		data?.slots.some((s) => (s.installed && !s.active) || (!s.installed && s.loaded)) ?? false
	);
	const full = $derived(!!data && data.usage.models >= data.usage.max_models);

	function mib(bytes: number): string {
		return (bytes / 1024 / 1024).toFixed(1);
	}
	function modelsFor(s: CoralSlot): CoralLibraryModel[] {
		return data?.library.filter((m) => m.info.slot === s) ?? [];
	}
	function slotName(s: CoralSlot): string {
		return t(`coralModels.slot.${s}`);
	}

	async function refresh() {
		error = '';
		try {
			data = await api.get<CoralModels>(`/coral/${providerId}/models`);
		} catch (err) {
			error = errorMessage(err);
		}
	}
	$effect(() => {
		if (providerId) void refresh();
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

	function add() {
		const meta = metaFile?.[0];
		const tpu = tpuFile?.[0];
		if (!meta || !tpu) {
			error = t('coralModels.filesRequired');
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
		void run(
			async () => {
				const model = await upload<CoralLibraryModel>(`/coral/${providerId}/library`, form);
				if (selectNow)
					await api.put(`/coral/${providerId}/slots/${model.info.slot}`, { model: model.id });
				name = license = source = '';
				allowed = false;
				metaFile = tpuFile = cpuFile = undefined;
			},
			selectNow ? t('coralModels.addedSelected') : t('coralModels.added')
		);
	}

	function select(s: CoralSlot, id: string) {
		if (!id) {
			if (!confirm(t('coralModels.confirmClear', { slot: slotName(s) }))) {
				void refresh();
				return;
			}
			void run(() => api.del(`/coral/${providerId}/slots/${s}`), t('coralModels.selected'));
			return;
		}
		void run(
			() => api.put(`/coral/${providerId}/slots/${s}`, { model: id }),
			t('coralModels.selected')
		);
	}

	function remove(m: CoralLibraryModel) {
		if (!confirm(t('common.confirmDelete', { name: m.info.name }))) return;
		void run(() => api.del(`/coral/${providerId}/library/${m.id}`), t('coralModels.deleted'));
	}

	function restart() {
		if (!confirm(t('coralModels.confirmRestart'))) return;
		void run(async () => {
			await api.post(`/coral/${providerId}/restart`);
			// The service needs a few seconds to load the models again.
			await new Promise((resolve) => setTimeout(resolve, 8000));
		}, t('coralModels.restarted'));
	}
</script>

<section
	class="space-y-3 rounded-lg border border-slate-200 p-3 dark:border-slate-700"
	data-testid="coral-models"
>
	<h3 class="font-medium">{t('coralModels.title')}</h3>
	<p class="hint">{t('coralModels.hint')}</p>
	<ErrorBox {error} />
	{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400" role="status">{info}</p>{/if}

	{#if data}
		<div class="grid gap-3 sm:grid-cols-3">
			{#each data.slots as s (s.slot)}
				<div>
					<label for="coral-slot-{s.slot}">{slotName(s.slot)}</label>
					<select
						id="coral-slot-{s.slot}"
						class="input"
						value={s.installed ? (s.model_id ?? '') : ''}
						onchange={(e) => select(s.slot, e.currentTarget.value)}
						disabled={busy}
						data-testid="coral-slot-{s.slot}"
					>
						<option value="">{t('coralModels.none')}</option>
						{#each modelsFor(s.slot) as m (m.id)}<option value={m.id}>{m.info.name}</option>{/each}
					</select>
					<p class="mt-1">
						{#if s.active}<span class="badge badge-ok">{t('coralModels.active')}</span>
						{:else if s.installed}<span class="badge badge-warn">{t('coralModels.pending')}</span>
						{:else}<span class="badge">{t('coralModels.empty')}</span>{/if}
					</p>
				</div>
			{/each}
		</div>
		{#if data.backend === 'mock'}<p class="hint">{t('coralModels.mockHint')}</p>{/if}
		{#if pending}
			<div class="flex flex-wrap items-center gap-3">
				<p class="hint">{t('coralModels.restartHint')}</p>
				<button
					type="button"
					class="btn btn-primary"
					onclick={restart}
					disabled={busy}
					data-testid="coral-models-restart">{t('coralModels.restart')}</button
				>
			</div>
		{/if}

		<h4 class="text-sm font-medium">{t('coralModels.libraryTitle')}</h4>
		<p class="hint">
			{t('coralModels.usage', {
				models: data.usage.models,
				max: data.usage.max_models,
				mib: mib(data.usage.bytes),
				maxMib: mib(data.usage.max_bytes)
			})}
		</p>
		{#if data.library.length}
			<div class="scroll-x">
				<table class="w-full text-sm" data-testid="coral-library">
					<thead class="text-left text-slate-500 dark:text-slate-400">
						<tr>
							<th class="py-1 pr-2 font-normal">{t('coralModels.model')}</th>
							<th class="py-1 pr-2 font-normal">{t('coralModels.slot')}</th>
							<th class="py-1 pr-2 font-normal">{t('coralModels.license')}</th>
							<th class="py-1 pr-2 text-right font-normal">MiB</th>
							<th class="py-1 font-normal"></th>
						</tr>
					</thead>
					<tbody>
						{#each data.library as m (m.id)}
							<tr class="border-t border-slate-200 align-top dark:border-slate-700">
								<td class="py-1 pr-2">
									{m.info.name}
									{#if m.selected}<span class="badge badge-ok ml-1">{t('coralModels.inUse')}</span
										>{/if}
									<br /><span class="font-mono text-xs break-all text-slate-500">{m.id}</span>
									{#if m.candidate}<span class="hint"> · {m.candidate}</span>{/if}
									<br /><span class="hint"
										>{formatDateTime(new Date(m.info.installed_at_unix * 1000).toISOString())}</span
									>
								</td>
								<td class="py-1 pr-2">{slotName(m.info.slot)}</td>
								<td class="py-1 pr-2 break-words">
									{#if m.info.license}{m.info.license}{#if m.info.source}<br /><span
												class="hint break-all">{m.info.source}</span
											>{/if}
									{:else}<span class="badge badge-warn">{t('coralModels.licenseUnknown')}</span
										>{/if}
								</td>
								<td class="py-1 pr-2 text-right">{mib(m.bytes)}</td>
								<td class="py-1 text-right">
									<button
										type="button"
										class="btn"
										onclick={() => remove(m)}
										disabled={busy || m.selected}
										title={m.selected ? t('coralModels.deleteInUse') : undefined}
										>{t('common.delete')}</button
									>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else}
			<p class="hint">{t('coralModels.libraryEmpty')}</p>
		{/if}
	{/if}

	<div class="space-y-3 border-t border-slate-200 pt-3 dark:border-slate-700">
		<h4 class="text-sm font-medium">{t('coralModels.uploadTitle')}</h4>
		<p class="hint">{t('coralModels.uploadHint')}</p>
		{#if full}<p class="hint text-amber-700 dark:text-amber-400">{t('coralModels.full')}</p>{/if}
		<div class="grid gap-3 sm:grid-cols-2">
			<div>
				<label for="coral-model-slot">{t('coralModels.slot')}</label>
				<select id="coral-model-slot" class="input" bind:value={slot} disabled={busy}>
					{#each slots as s (s)}<option value={s}>{slotName(s)}</option>{/each}
				</select>
			</div>
			<div>
				<label for="coral-model-name">{t('coralModels.name')}</label>
				<input
					id="coral-model-name"
					class="input"
					maxlength="200"
					placeholder="Citrinet 256 DE 2026-11"
					bind:value={name}
					disabled={busy}
				/>
			</div>
			<div>
				<label for="coral-model-license">{t('coralModels.license')}</label>
				<input
					id="coral-model-license"
					class="input"
					maxlength="200"
					placeholder="CC-BY-4.0"
					bind:value={license}
					disabled={busy}
				/>
			</div>
			<div>
				<label for="coral-model-source">{t('coralModels.source')}</label>
				<input
					id="coral-model-source"
					class="input"
					maxlength="200"
					placeholder="https://huggingface.co/…"
					bind:value={source}
					disabled={busy}
				/>
			</div>
			<div>
				<label for="coral-model-json">model.json</label>
				<input
					id="coral-model-json"
					class="input"
					type="file"
					accept=".json,application/json"
					bind:files={metaFile}
					disabled={busy}
				/>
			</div>
			<div>
				<label for="coral-model-tpu">model_edgetpu.tflite</label>
				<input
					id="coral-model-tpu"
					class="input"
					type="file"
					accept=".tflite"
					bind:files={tpuFile}
					disabled={busy}
				/>
			</div>
			<div>
				<label for="coral-model-cpu"
					>model.tflite <span class="hint">({t('coralModels.optional')})</span></label
				>
				<input
					id="coral-model-cpu"
					class="input"
					type="file"
					accept=".tflite"
					bind:files={cpuFile}
					disabled={busy}
				/>
			</div>
		</div>
		<label class="flex items-start gap-2 text-sm"
			><input type="checkbox" bind:checked={selectNow} disabled={busy} /><span
				>{t('coralModels.selectNow')}</span
			></label
		>
		<label class="flex items-start gap-2 text-sm"
			><input type="checkbox" bind:checked={allowed} disabled={busy} /><span
				>{t('coralModels.confirmLicense')}</span
			></label
		>
		<button
			type="button"
			class="btn btn-primary"
			onclick={add}
			disabled={busy || full || !allowed || !name.trim() || !license.trim() || !source.trim()}
			data-testid="coral-models-install">{t('coralModels.install')}</button
		>
	</div>
</section>
