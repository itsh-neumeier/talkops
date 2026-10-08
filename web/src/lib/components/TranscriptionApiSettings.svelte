<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type TranscriptionApi } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	/** OpenAI-compatible providers; models are suggestions, any may be typed. */
	const providers = [
		{
			id: 'openai',
			url: 'https://api.openai.com/v1',
			models: ['whisper-1', 'gpt-4o-transcribe', 'gpt-4o-mini-transcribe']
		},
		{
			id: 'groq',
			url: 'https://api.groq.com/openai/v1',
			models: ['whisper-large-v3-turbo', 'whisper-large-v3']
		},
		{ id: 'mistral', url: 'https://api.mistral.ai/v1', models: ['voxtral-mini-latest'] },
		{
			id: 'custom',
			url: 'http://192.168.1.10:8000/v1',
			models: ['Systran/faster-whisper-large-v3', 'deepdml/faster-whisper-large-v3-turbo-ct2']
		}
	] as const;
	type ProviderId = (typeof providers)[number]['id'];

	let cfg = $state<TranscriptionApi | null>(null);
	let url = $state('');
	let model = $state('');
	let key = $state('');
	let error = $state('');
	let info = $state('');
	let busy = $state(false);

	const provider = $derived<ProviderId>(
		providers.find((p) => p.id !== 'custom' && url.trim().replace(/\/$/, '') === p.url)?.id ??
			'custom'
	);
	const suggestions = $derived(providers.find((p) => p.id === provider)?.models ?? []);

	onMount(async () => {
		try {
			cfg = await api.get<TranscriptionApi>('/settings/transcription-api');
			url = cfg.url;
			model = cfg.model;
		} catch (err) {
			error = errorMessage(err);
		}
	});

	function pick(id: ProviderId) {
		const p = providers.find((p) => p.id === id)!;
		url = id === 'custom' ? '' : p.url;
		model = p.models[0];
	}

	async function save() {
		error = '';
		info = '';
		try {
			cfg = await api.put<TranscriptionApi>('/settings/transcription-api', {
				url: url.trim(),
				model: model.trim(),
				key: key ? key : null
			});
			key = '';
			info = t('common.saved');
			return true;
		} catch (err) {
			error = errorMessage(err);
			return false;
		}
	}

	async function removeKey() {
		error = '';
		try {
			cfg = await api.put<TranscriptionApi>('/settings/transcription-api', {
				url: url.trim(),
				model: model.trim(),
				key: ''
			});
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function test() {
		if (!(await save())) return;
		busy = true;
		info = '';
		try {
			const r = await api.post<{ model_found: boolean | null; speech_models: string[] }>(
				'/settings/transcription-api/test',
				{}
			);
			info =
				r.model_found === false
					? t('stt.testNoModel', { model, models: r.speech_models.join(', ') || '–' })
					: r.model_found
						? t('stt.testOk')
						: t('stt.testReachable');
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<!-- Inside the settings form, so no <form> of its own. -->
<div class="space-y-3 rounded-lg border border-slate-200 p-3 dark:border-slate-700">
	<h3 class="font-medium">{t('stt.apiTitle')}</h3>
	<p
		class="rounded-md bg-amber-50 p-2 text-sm text-amber-900 dark:bg-amber-950/40 dark:text-amber-200"
	>
		{t('stt.privacy')}
	</p>
	<ErrorBox {error} />
	{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400" data-testid="stt-info">
			{info}
		</p>{/if}
	<div>
		<label for="stt-provider">{t('stt.provider')}</label>
		<select
			id="stt-provider"
			class="input"
			value={provider}
			onchange={(e) => pick((e.currentTarget as HTMLSelectElement).value as ProviderId)}
		>
			{#each providers as p (p.id)}
				<option value={p.id}>{t(`stt.provider.${p.id}`)}</option>
			{/each}
		</select>
	</div>
	<div>
		<label for="stt-url">{t('stt.url')}</label>
		<input
			id="stt-url"
			class="input font-mono"
			bind:value={url}
			required
			placeholder="https://…/v1"
		/>
		<p class="hint">{t('stt.urlHint')}</p>
	</div>
	<div>
		<label for="stt-model">{t('stt.model')}</label>
		<input id="stt-model" class="input font-mono" list="stt-models" bind:value={model} required />
		<datalist id="stt-models">
			{#each suggestions as m (m)}<option value={m}></option>{/each}
		</datalist>
		<p class="hint">{t('stt.modelHint')}</p>
	</div>
	<div>
		<label for="stt-key">{t('stt.key')}</label>
		<input
			id="stt-key"
			class="input font-mono"
			type="password"
			autocomplete="off"
			bind:value={key}
			placeholder={cfg?.has_key ? t('stt.keyStored') : t('stt.keyNone')}
		/>
		{#if cfg?.has_key}
			<button
				type="button"
				class="mt-1 text-sm text-red-700 hover:underline dark:text-red-400"
				onclick={removeKey}>{t('stt.keyRemove')}</button
			>
		{/if}
	</div>
	<div class="flex justify-end gap-2">
		<button type="button" class="btn" disabled={busy} onclick={test}>{t('stt.test')}</button>
		<button type="button" class="btn btn-primary" onclick={save}>{t('stt.saveApi')}</button>
	</div>
</div>
