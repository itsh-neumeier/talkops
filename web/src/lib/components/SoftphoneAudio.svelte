<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import { setAudioOutput, toneUrl } from '#lib/softphone.ts';

	let {
		microphone = $bindable(''),
		output = $bindable(''),
		disabled = false,
		active = true
	} = $props<{
		microphone?: string;
		output?: string;
		disabled?: boolean;
		active?: boolean;
	}>();
	let devices = $state<MediaDeviceInfo[]>([]);
	let error = $state('');
	let testing = $state(false);
	let level = $state(0);
	let supported = $state(false);
	let stream: MediaStream | undefined;
	let context: AudioContext | undefined;
	let frame = 0;
	let generation = 0;
	let testAudio: HTMLAudioElement | undefined;

	async function refresh() {
		try {
			devices = await navigator.mediaDevices.enumerateDevices();
			if (
				microphone &&
				!devices.some((d) => d.kind === 'audioinput' && d.deviceId === microphone)
			) {
				microphone = '';
				error = t('phone.deviceLost');
			}
			if (output && !devices.some((d) => d.kind === 'audiooutput' && d.deviceId === output)) {
				output = '';
				error = t('phone.deviceLost');
			}
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function stop() {
		generation++;
		cancelAnimationFrame(frame);
		stream?.getTracks().forEach((track) => track.stop());
		stream = undefined;
		void context?.close();
		context = undefined;
		testing = false;
		level = 0;
		testAudio?.pause();
		if (testAudio) URL.revokeObjectURL(testAudio.src);
		testAudio = undefined;
	}

	async function testMicrophone() {
		if (testing) {
			stop();
			return;
		}
		error = '';
		const current = ++generation;
		try {
			const media = await navigator.mediaDevices.getUserMedia({
				audio: microphone ? { deviceId: { exact: microphone } } : true
			});
			if (current !== generation || disabled || !active) {
				media.getTracks().forEach((t) => t.stop());
				return;
			}
			stream = media;
			context = new AudioContext();
			const analyser = context.createAnalyser();
			analyser.fftSize = 256;
			context.createMediaStreamSource(media).connect(analyser);
			await context.resume();
			if (current !== generation || disabled || !active) return;
			const samples = new Uint8Array(analyser.fftSize);
			testing = true;
			const measure = () => {
				analyser.getByteTimeDomainData(samples);
				level = Math.min(
					100,
					Math.sqrt(samples.reduce((sum, s) => sum + ((s - 128) / 128) ** 2, 0) / samples.length) *
						300
				);
				frame = requestAnimationFrame(measure);
			};
			measure();
			await refresh();
		} catch (err) {
			stop();
			error = errorMessage(err);
		}
	}

	async function testSound() {
		error = '';
		try {
			testAudio?.pause();
			if (testAudio) URL.revokeObjectURL(testAudio.src);
			testAudio = new Audio(toneUrl());
			await setAudioOutput(testAudio, output);
			await testAudio.play();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	onMount(() => {
		supported = 'setSinkId' in HTMLMediaElement.prototype;
		void refresh();
		navigator.mediaDevices?.addEventListener('devicechange', refresh);
		return () => navigator.mediaDevices?.removeEventListener('devicechange', refresh);
	});
	onDestroy(stop);
	$effect(() => {
		if (disabled || !active) stop();
	});
</script>

<details class="card space-y-3">
	<summary class="cursor-pointer font-medium">{t('phone.audioSettings')}</summary>
	<label class="block" for="phone-microphone">{t('phone.microphone')}</label>
	<select id="phone-microphone" class="input" bind:value={microphone} {disabled} onchange={stop}>
		<option value="">{t('phone.systemDefault')}</option>
		{#each devices.filter((d) => d.kind === 'audioinput' && d.deviceId) as device, i (device.deviceId)}
			<option value={device.deviceId}>{device.label || `${t('phone.microphone')} ${i + 1}`}</option>
		{/each}
	</select>
	<label class="block" for="phone-output">{t('phone.output')}</label>
	<select id="phone-output" class="input" bind:value={output} disabled={!supported}>
		<option value="">{t('phone.systemDefault')}</option>
		{#each devices.filter((d) => d.kind === 'audiooutput' && d.deviceId) as device, i (device.deviceId)}
			<option value={device.deviceId}>{device.label || `${t('phone.output')} ${i + 1}`}</option>
		{/each}
	</select>
	{#if !supported}<p class="hint">{t('phone.outputUnsupported')}</p>{/if}
	<div class="flex flex-wrap gap-2">
		<button class="btn btn-sm" {disabled} onclick={testMicrophone}
			>{testing ? t('phone.stopTest') : t('phone.testMicrophone')}</button
		>
		<button class="btn btn-sm" onclick={testSound}>{t('phone.testSound')}</button>
	</div>
	{#if testing}<meter
			class="w-full"
			min="0"
			max="100"
			value={level}
			aria-label={t('phone.microphoneLevel')}
		></meter>{/if}
	<p class="hint">{t('phone.microphoneHelp')}</p>
	{#if error}<p role="alert" class="text-sm text-red-600">{error}</p>{/if}
</details>
