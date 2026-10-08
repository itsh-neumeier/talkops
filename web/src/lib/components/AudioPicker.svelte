<script lang="ts" module>
	export type AudioMode = 'default' | 'generate' | 'record' | 'upload' | 'none';
</script>

<script lang="ts">
	// Chooses the audio for a greeting or prompt, like UniFi Talk: system
	// default, generated with a computer voice, recorded in the browser,
	// uploaded, or none. Every choice except default/none becomes a clip
	// (`clipId`); generated text is rendered on demand and can be listened to
	// before saving.
	import { onDestroy } from 'svelte';
	import { api } from '#lib/api.ts';
	import {
		clipUrl,
		formatDuration,
		record,
		toWav,
		uploadClip,
		type Clip,
		type Recording,
		type Voice
	} from '#lib/audio.ts';
	import AudioPlayer from '#lib/components/AudioPlayer.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let {
		mode = $bindable(),
		clipId = $bindable(),
		modes = ['default', 'generate', 'record', 'upload', 'none'],
		language = 'de',
		defaultText = '',
		legacy = null,
		hints = {}
	}: {
		mode: AudioMode;
		clipId: string | null;
		modes?: AudioMode[];
		language?: 'de' | 'en';
		defaultText?: string;
		/** A greeting stored before clips existed, shown while no clip is chosen. */
		legacy?: { mode: AudioMode; src: string; text?: string } | null;
		/** Extra explanation per mode. */
		hints?: Partial<Record<AudioMode, string>>;
	} = $props();

	const SOURCE_MODE: Record<Clip['source'], AudioMode> = {
		tts: 'generate',
		recording: 'record',
		upload: 'upload'
	};
	const ids = $props.id();
	let voices = $state<Voice[]>([]);
	let clip = $state<Clip | null>(null);
	let text = $state('');
	let lang = $state<'de' | 'en'>('de');
	let voice = $state<1 | 2>(1);
	let busy = $state(false);
	let error = $state('');
	let recording = $state<Recording | null>(null);
	let recordStart = $state(0);
	let now = $state(0);
	let timer: ReturnType<typeof setInterval> | undefined;

	// Initial state from the current clip (or the legacy greeting).
	let loadedFor: string | null | undefined;
	$effect(() => {
		const id = clipId;
		if (id === loadedFor) return;
		loadedFor = id;
		if (!id) {
			clip = null;
			text = legacy?.text || defaultText;
			lang = language;
			return;
		}
		if (clip?.id === id) return;
		const initial = clip === null;
		api
			.get<Clip>(`/audio/clips/${id}`)
			.then((c) => {
				clip = c;
				if (initial) mode = SOURCE_MODE[c.source];
				if (c.source === 'tts') {
					text = c.text;
					lang = c.language;
					voice = c.voice;
				} else if (!text) {
					text = defaultText;
					lang = language;
				}
			})
			.catch((err) => (error = errorMessage(err)));
	});

	$effect(() => {
		api
			.get<Voice[]>('/audio/voices')
			.then((v) => (voices = v))
			.catch(() => {});
	});

	// While a generated clip renders: poll its status.
	$effect(() => {
		if (clip?.status !== 'pending') return;
		const id = clip.id;
		const handle = setTimeout(async () => {
			try {
				const c = await api.get<Clip>(`/audio/clips/${id}`);
				if (clip?.id === id) clip = c;
			} catch (err) {
				error = errorMessage(err);
			}
		}, 1000);
		return () => clearTimeout(handle);
	});

	const langVoices = $derived(voices.filter((v) => v.language === lang));
	// The clip matching the visible mode; a generated clip only while the text
	// is unchanged.
	const shown = $derived.by(() => {
		if (!clip) return null;
		if (mode === 'generate')
			return clip.source === 'tts' &&
				clip.text === normalize(text) &&
				clip.language === lang &&
				clip.voice === voice
				? clip
				: null;
		if (mode === 'record') return clip.source === 'recording' ? clip : null;
		if (mode === 'upload') return clip.source === 'upload' ? clip : null;
		return null;
	});
	const generatedChanged = $derived(
		mode === 'generate' && clip?.source === 'tts' && shown === null
	);

	function normalize(s: string) {
		return s.split(/\s+/).filter(Boolean).join(' ');
	}

	function pick(m: AudioMode) {
		error = '';
		mode = m;
		if (m === 'generate' && !text) text = defaultText;
	}

	function useClip(c: Clip) {
		clip = c;
		loadedFor = c.id;
		clipId = c.id;
	}

	async function generate(): Promise<Clip> {
		const c = await api.post<Clip>('/audio/clips/tts', { text, language: lang, voice });
		useClip(c);
		return c;
	}

	async function onGenerate() {
		error = '';
		busy = true;
		try {
			await generate();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	/**
	 * Called before saving: the clip for the chosen mode (generating the text
	 * if it has not been generated yet), or null for default/none or a kept
	 * legacy greeting.
	 */
	export async function ensure(): Promise<string | null> {
		if (mode === 'default' || mode === 'none') return null;
		if (mode === 'generate') {
			if (!normalize(text)) throw new Error(t('audio.textRequired'));
			return (shown ?? (await generate())).id;
		}
		if (shown) return shown.id;
		if (legacy?.mode === mode) return null;
		throw new Error(mode === 'record' ? t('audio.recordFirst') : t('audio.uploadFirst'));
	}

	async function startRecording() {
		error = '';
		try {
			recording = await record();
			recordStart = Date.now();
			now = recordStart;
			timer = setInterval(() => {
				now = Date.now();
				if (now - recordStart > 10 * 60 * 1000) void stopRecording();
			}, 250);
		} catch (err) {
			error = t('audio.micDenied', { error: errorMessage(err) });
		}
	}

	async function stopRecording() {
		const rec = recording;
		if (!rec) return;
		clearInterval(timer);
		recording = null;
		busy = true;
		try {
			useClip(await uploadClip(await rec.stop(), 'recording'));
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	async function onFile(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;
		error = '';
		busy = true;
		try {
			const wav = await toWav(file).catch(() => {
				throw new Error(t('audio.unsupportedFile'));
			});
			useClip(await uploadClip(wav, 'upload'));
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
			input.value = '';
		}
	}

	onDestroy(() => {
		clearInterval(timer);
		recording?.cancel();
	});

	const labels: Record<AudioMode, MessageKey> = {
		default: 'audio.mode.default',
		generate: 'audio.mode.generate',
		record: 'audio.mode.record',
		upload: 'audio.mode.upload',
		none: 'audio.mode.none'
	};
</script>

<div class="space-y-3">
	<div
		class="inline-flex flex-wrap rounded-lg border border-slate-300 p-0.5 dark:border-slate-600"
		role="radiogroup"
	>
		{#each modes as m (m)}
			<button
				type="button"
				role="radio"
				aria-checked={mode === m}
				class="rounded-md px-3 py-1 text-sm {mode === m
					? 'bg-teal-600 text-white'
					: 'text-slate-600 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-700'}"
				onclick={() => pick(m)}>{t(labels[m])}</button
			>
		{/each}
	</div>
	<ErrorBox {error} />

	{#if hints[mode]}<p class="hint">{hints[mode]}</p>{/if}

	{#if mode === 'generate'}
		<div class="space-y-2">
			<div class="grid gap-3 sm:grid-cols-2">
				<div>
					<label for="{ids}-lang">{t('audio.language')}</label>
					<select id="{ids}-lang" class="input" bind:value={lang} onchange={() => (voice = 1)}>
						<option value="de">Deutsch</option>
						<option value="en">English</option>
					</select>
				</div>
				<div>
					<label for="{ids}-voice">{t('audio.voice')}</label>
					<select id="{ids}-voice" class="input" bind:value={voice}>
						{#each langVoices as v (v.voice)}
							<option value={v.voice}
								>{t('audio.voiceN', { n: v.voice })} – {v.name} ({t(
									v.gender === 'female' ? 'audio.female' : 'audio.male'
								)})</option
							>
						{/each}
					</select>
				</div>
			</div>
			<label for="{ids}-text">{t('audio.text')}</label>
			<textarea id="{ids}-text" class="input" rows="3" maxlength="1000" bind:value={text}
			></textarea>
			<div class="flex items-center justify-between gap-2">
				<span class="text-xs text-slate-500">{text.length} / 1000</span>
				<button
					type="button"
					class="btn btn-sm"
					onclick={onGenerate}
					disabled={busy || !normalize(text) || shown?.status === 'pending'}
				>
					{shown || generatedChanged ? t('audio.regenerate') : t('audio.generate')}
				</button>
			</div>
			{#if generatedChanged}
				<p class="hint">{t('audio.changed')}</p>
			{/if}
		</div>
	{:else if mode === 'record'}
		<div class="flex flex-wrap items-center gap-3">
			{#if recording}
				<button type="button" class="btn btn-danger btn-sm" onclick={stopRecording}>
					■ {t('audio.stop')}
				</button>
				<span class="flex items-center gap-2 font-mono text-sm text-red-600">
					<span class="h-2.5 w-2.5 animate-pulse rounded-full bg-red-600"></span>
					{formatDuration(now - recordStart)}
				</span>
			{:else}
				<button type="button" class="btn btn-sm" onclick={startRecording} disabled={busy}>
					● {shown ? t('audio.recordAgain') : t('audio.record')}
				</button>
			{/if}
		</div>
	{:else if mode === 'upload'}
		<div>
			<label for="{ids}-file">{t('audio.file')}</label>
			<input
				id="{ids}-file"
				class="input"
				type="file"
				accept="audio/*,.wav,.mp3,.ogg,.m4a"
				onchange={onFile}
				disabled={busy}
			/>
			<p class="hint">{t('audio.fileHint')}</p>
		</div>
	{/if}

	{#if busy}
		<p class="text-sm text-slate-500">{t('audio.working')}</p>
	{/if}
	{#if mode === 'generate' || mode === 'record' || mode === 'upload'}
		{#if shown?.status === 'pending'}
			<p class="text-sm text-slate-500">{t('audio.rendering')}</p>
		{:else if shown?.status === 'failed'}
			<p class="text-sm text-red-600">{t('audio.failed')}</p>
		{:else if shown?.status === 'ready'}
			<AudioPlayer src={clipUrl(shown.id)} />
		{:else if !shown && !generatedChanged && legacy?.mode === mode}
			<AudioPlayer src={legacy.src} />
		{/if}
	{/if}
</div>
