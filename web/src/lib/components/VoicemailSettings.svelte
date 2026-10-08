<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type VoicemailBox } from '#lib/api.ts';
	import AudioPicker, { type AudioMode } from '#lib/components/AudioPicker.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let { extensionId, displayName = '' }: { extensionId: string; displayName?: string } = $props();

	let box = $state<VoicemailBox | null>(null);
	let pin = $state('');
	let error = $state('');
	let saved = $state(false);
	let mode = $state<AudioMode>('default');
	let clipId = $state<string | null>(null);
	let picker = $state<ReturnType<typeof AudioPicker>>();
	let pickerKey = $state(0);

	// Greetings from before audio clips: rendered text or recorded by phone.
	const legacy = $derived.by(() => {
		if (!box || box.greeting_status !== 'ready') return null;
		const src = `/api/v1/extensions/${extensionId}/voicemail/greeting?v=${pickerKey}`;
		if (box.greeting === 'tts') return { mode: 'generate' as const, src, text: box.greeting_text };
		if (box.greeting === 'recorded') return { mode: 'record' as const, src };
		return null;
	});

	function apply(b: VoicemailBox) {
		box = b;
		clipId = b.greeting === 'clip' ? b.greeting_clip_id : null;
		mode =
			b.greeting === 'tts' || b.greeting === 'clip'
				? 'generate'
				: b.greeting === 'recorded'
					? 'record'
					: b.greeting;
		pickerKey++;
	}

	async function load() {
		try {
			apply(await api.get<VoicemailBox>(`/extensions/${extensionId}/voicemail`));
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	// Refresh while a greeting from before clips is being rendered.
	$effect(() => {
		if (box?.greeting_status !== 'pending') return;
		const timer = setTimeout(load, 3000);
		return () => clearTimeout(timer);
	});

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!box) return;
		error = '';
		saved = false;
		try {
			const clip = (await picker?.ensure()) ?? null;
			const greeting = mode === 'default' || mode === 'none' ? mode : clip ? 'clip' : box.greeting;
			const updated = await api.put<VoicemailBox>(`/extensions/${extensionId}/voicemail`, {
				enabled: box.enabled,
				pin: pin === '' ? null : pin,
				email_notify: box.email_notify,
				attach_audio: box.attach_audio,
				language: box.language || null,
				greeting,
				greeting_text: box.greeting_text,
				greeting_clip_id: clip,
				max_message_secs: Number(box.max_message_secs)
			});
			apply(updated);
			pin = '';
			saved = true;
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<form class="space-y-3" onsubmit={save}>
	<h2>{t('vm.settings')}</h2>
	<ErrorBox {error} />
	{#if box}
		<label class="flex items-center gap-2">
			<input type="checkbox" bind:checked={box.enabled} />
			{t('vm.enabled')}
		</label>
		<p class="hint">{t('vm.enabledHint')}</p>
		<div class="grid gap-3 sm:grid-cols-3">
			<div>
				<label for="vm-pin-{extensionId}">{t('vm.pin')}</label>
				<input
					id="vm-pin-{extensionId}"
					class="input font-mono"
					inputmode="numeric"
					autocomplete="off"
					pattern="[0-9]{'{'}4,10{'}'}"
					bind:value={pin}
					placeholder={box.has_pin ? t('vm.pinKeep') : t('vm.pinNone')}
				/>
			</div>
			<div>
				<label for="vm-lang-{extensionId}">{t('vm.language')}</label>
				<select id="vm-lang-{extensionId}" class="input" bind:value={box.language}>
					<option value={null}>{t('vm.languageDefault')}</option>
					<option value="de">Deutsch</option>
					<option value="en">English</option>
				</select>
			</div>
			<div>
				<label for="vm-max-{extensionId}">{t('vm.maxLength')}</label>
				<input
					id="vm-max-{extensionId}"
					class="input"
					type="number"
					min="10"
					max="600"
					bind:value={box.max_message_secs}
				/>
			</div>
		</div>
		<p class="hint">{t('vm.pinHint')}</p>

		<fieldset class="space-y-2">
			<legend class="text-sm font-medium">{t('vm.greeting')}</legend>
			{#key pickerKey}
				<AudioPicker
					bind:this={picker}
					bind:mode
					bind:clipId
					language={box.language ?? 'de'}
					defaultText={t('vm.greetingTemplate', { name: displayName || '…' })}
					{legacy}
					hints={{
						default: t('vm.greetingDefaultHint'),
						record: t('vm.greetingRecordHint'),
						none: t('vm.greetingNoneHint')
					}}
				/>
			{/key}
		</fieldset>

		<label class="flex items-center gap-2">
			<input type="checkbox" bind:checked={box.email_notify} />
			{t('vm.emailNotify')}
		</label>
		{#if box.email_notify}
			<label class="flex items-center gap-2 pl-6">
				<input type="checkbox" bind:checked={box.attach_audio} />
				{t('vm.attachAudio')}
			</label>
		{/if}
		<p class="hint">{t('vm.emailHint')}</p>

		<div class="flex items-center justify-end gap-3">
			{#if saved}<span class="text-sm text-emerald-700 dark:text-emerald-400"
					>{t('common.saved')}</span
				>{/if}
			<button class="btn btn-primary">{t('common.save')}</button>
		</div>
	{/if}
</form>
