<script lang="ts">
	import { api, upload, type CoralModelSlot } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	/** Coral connection whose service stores the models (ADR 0048). */
	let { providerId }: { providerId: string } = $props();

	const slots = ['de', 'en', 'langid'] as const;
	let models = $state<CoralModelSlot[]>([]);
	let backend = $state('');
	let error = $state('');
	let info = $state('');
	let busy = $state(false);
	// Upload form: inside the settings form, so no <form> of its own.
	let slot = $state<(typeof slots)[number]>('de');
	let license = $state('');
	let source = $state('');
	let allowed = $state(false);
	let metaFile = $state<FileList>();
	let tpuFile = $state<FileList>();
	let cpuFile = $state<FileList>();

	const pending = $derived(
		models.some((m) => (m.installed && !m.active) || (!m.installed && m.backup))
	);

	async function refresh() {
		error = '';
		try {
			const r = await api.get<{ backend: string; models: CoralModelSlot[] }>(
				`/coral/${providerId}/models`
			);
			models = r.models;
			backend = r.backend;
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

	function install() {
		const meta = metaFile?.[0];
		const tpu = tpuFile?.[0];
		if (!meta || !tpu) {
			error = t('coralModels.filesRequired');
			return;
		}
		const form = new FormData();
		form.append('license', license);
		form.append('source', source);
		form.append('model_json', meta);
		form.append('model_edgetpu', tpu);
		const cpu = cpuFile?.[0];
		if (cpu) form.append('model_cpu', cpu);
		void run(async () => {
			await upload(`/coral/${providerId}/models/${slot}`, form);
			license = source = '';
			allowed = false;
			metaFile = tpuFile = cpuFile = undefined;
		}, t('coralModels.installed'));
	}

	function restore(name: CoralModelSlot['slot']) {
		if (!confirm(t('coralModels.confirmRestore', { slot: t(`coralModels.slot.${name}`) }))) return;
		void run(
			() => api.post(`/coral/${providerId}/models/${name}/restore`),
			t('coralModels.restored')
		);
	}

	function remove(name: CoralModelSlot['slot']) {
		if (!confirm(t('coralModels.confirmRemove', { slot: t(`coralModels.slot.${name}`) }))) return;
		void run(() => api.del(`/coral/${providerId}/models/${name}`), t('coralModels.removed'));
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

	{#if models.length}
		<div class="scroll-x">
			<table class="w-full text-sm">
				<thead class="text-left text-slate-500 dark:text-slate-400">
					<tr>
						<th class="py-1 pr-2 font-normal">{t('coralModels.slot')}</th>
						<th class="py-1 pr-2 font-normal">{t('coralModels.model')}</th>
						<th class="py-1 pr-2 font-normal">{t('coralModels.license')}</th>
						<th class="py-1 pr-2 font-normal">{t('coralModels.state')}</th>
						<th class="py-1 font-normal"></th>
					</tr>
				</thead>
				<tbody>
					{#each models as m (m.slot)}
						<tr class="border-t border-slate-200 align-top dark:border-slate-700">
							<td class="py-1 pr-2">{t(`coralModels.slot.${m.slot}`)}</td>
							<td class="py-1 pr-2">
								{#if m.installed}
									{m.candidate ?? '–'}
									{#if m.tpu_sha256}<br /><span class="font-mono text-xs break-all text-slate-500"
											>{m.tpu_sha256.slice(0, 16)}</span
										>{/if}
									{#if m.info}<br /><span class="hint"
											>{formatDateTime(
												new Date(m.info.installed_at_unix * 1000).toISOString()
											)}</span
										>{/if}
								{:else}–{/if}
							</td>
							<td class="py-1 pr-2 break-words">
								{#if m.info}{m.info.license}<br /><span class="hint break-all">{m.info.source}</span
									>{:else if m.meta_license}{m.meta_license}{:else if m.installed}<span
										class="badge badge-warn">{t('coralModels.licenseUnknown')}</span
									>{/if}
							</td>
							<td class="py-1 pr-2">
								{#if m.active}<span class="badge badge-ok">{t('coralModels.active')}</span>
								{:else if m.installed}<span class="badge badge-warn"
										>{t('coralModels.pending')}</span
									>
								{:else}<span class="badge">{t('coralModels.empty')}</span>{/if}
							</td>
							<td class="py-1 text-right whitespace-nowrap">
								{#if m.backup}<button
										type="button"
										class="btn"
										onclick={() => restore(m.slot)}
										disabled={busy}>{t('coralModels.restore')}</button
									>{/if}
								{#if m.installed}<button
										type="button"
										class="btn"
										onclick={() => remove(m.slot)}
										disabled={busy}>{t('common.delete')}</button
									>{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		{#if backend === 'mock'}<p class="hint">{t('coralModels.mockHint')}</p>{/if}
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
	{/if}

	<div class="space-y-3 border-t border-slate-200 pt-3 dark:border-slate-700">
		<h4 class="text-sm font-medium">{t('coralModels.uploadTitle')}</h4>
		<p class="hint">{t('coralModels.uploadHint')}</p>
		<div class="grid gap-3 sm:grid-cols-2">
			<div>
				<label for="coral-model-slot">{t('coralModels.slot')}</label>
				<select id="coral-model-slot" class="input" bind:value={slot} disabled={busy}>
					{#each slots as s (s)}<option value={s}>{t(`coralModels.slot.${s}`)}</option>{/each}
				</select>
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
			<div class="sm:col-span-2">
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
			><input type="checkbox" bind:checked={allowed} disabled={busy} /><span
				>{t('coralModels.confirmLicense')}</span
			></label
		>
		<button
			type="button"
			class="btn btn-primary"
			onclick={install}
			disabled={busy || !allowed || !license.trim() || !source.trim()}
			data-testid="coral-models-install">{t('coralModels.install')}</button
		>
	</div>
</section>
