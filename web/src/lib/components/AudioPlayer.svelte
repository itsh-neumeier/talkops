<script lang="ts">
	// Compact player with a waveform: play/pause, seek by clicking the bars,
	// elapsed time and playback speed.
	import { decode, fetchAudio, formatDuration, peaks } from '#lib/audio.ts';
	import { t } from '#lib/i18n/index.svelte.ts';

	let { src, bars = 64 }: { src: string; bars?: number } = $props();

	let audio = $state<HTMLAudioElement>();
	let url = $state('');
	let levels = $state<number[]>([]);
	let failed = $state(false);
	let playing = $state(false);
	let current = $state(0);
	let duration = $state(0);
	let speed = $state(1);

	$effect(() => {
		const source = src;
		let objectUrl = '';
		let cancelled = false;
		failed = false;
		levels = [];
		playing = false;
		current = 0;
		fetchAudio(source)
			.then(async (blob) => {
				if (cancelled) return;
				objectUrl = URL.createObjectURL(blob);
				url = objectUrl;
				const buffer = await decode(await blob.arrayBuffer());
				if (!cancelled) {
					levels = peaks(buffer, bars);
					duration = buffer.duration;
				}
			})
			.catch(() => {
				if (!cancelled) failed = true;
			});
		return () => {
			cancelled = true;
			if (objectUrl) URL.revokeObjectURL(objectUrl);
		};
	});

	$effect(() => {
		if (audio) audio.playbackRate = speed;
	});

	function toggle() {
		if (!audio) return;
		if (audio.paused) void audio.play();
		else audio.pause();
	}

	function seek(i: number) {
		if (!audio || !duration) return;
		audio.currentTime = (i / bars) * duration;
		current = audio.currentTime;
	}

	function nextSpeed() {
		speed = speed === 1 ? 1.5 : speed === 1.5 ? 2 : 1;
	}

	const progress = $derived(duration ? current / duration : 0);
</script>

{#if failed}
	<p class="text-sm text-red-600">{t('audio.loadFailed')}</p>
{:else}
	<div
		class="flex items-center gap-3 rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 dark:border-slate-700 dark:bg-slate-800/60"
	>
		<button
			type="button"
			class="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-teal-600 text-white hover:bg-teal-700 disabled:opacity-50"
			onclick={toggle}
			disabled={!url}
			aria-label={playing ? t('audio.pause') : t('audio.play')}
		>
			{#if playing}
				<svg viewBox="0 0 16 16" class="h-3.5 w-3.5" fill="currentColor"
					><rect x="3" y="2" width="4" height="12" rx="1" /><rect
						x="9"
						y="2"
						width="4"
						height="12"
						rx="1"
					/></svg
				>
			{:else}
				<svg viewBox="0 0 16 16" class="ml-0.5 h-3.5 w-3.5" fill="currentColor"
					><path d="M4 2.5v11l9-5.5z" /></svg
				>
			{/if}
		</button>
		<div
			class="flex h-8 min-w-0 flex-1 items-center gap-px"
			role="group"
			aria-label={t('audio.waveform')}
		>
			{#if levels.length === 0}
				<div class="h-px w-full bg-slate-300 dark:bg-slate-600"></div>
			{/if}
			{#each levels as level, i (i)}
				<button
					type="button"
					tabindex="-1"
					class="h-full flex-1 cursor-pointer"
					onclick={() => seek(i)}
					aria-hidden="true"
				>
					<span
						class="block w-full rounded-sm {i / levels.length < progress
							? 'bg-teal-600 dark:bg-teal-400'
							: 'bg-slate-300 dark:bg-slate-600'}"
						style="height: {Math.max(8, level * 100)}%"
					></span>
				</button>
			{/each}
		</div>
		<span class="shrink-0 font-mono text-xs text-slate-500 tabular-nums">
			{formatDuration(current * 1000)} / {formatDuration(duration * 1000)}
		</span>
		<button
			type="button"
			class="shrink-0 rounded border border-slate-300 px-1.5 font-mono text-xs dark:border-slate-600"
			onclick={nextSpeed}
			title={t('audio.speed')}>{speed}×</button
		>
		<audio
			bind:this={audio}
			src={url || undefined}
			preload="auto"
			onplay={() => (playing = true)}
			onpause={() => (playing = false)}
			onended={() => {
				playing = false;
				current = 0;
			}}
			ontimeupdate={() => (current = audio?.currentTime ?? 0)}
		></audio>
	</div>
{/if}
