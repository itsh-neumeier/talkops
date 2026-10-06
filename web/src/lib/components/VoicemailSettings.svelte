<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type VoicemailBox } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let { extensionId, displayName = '' }: { extensionId: string; displayName?: string } = $props();

	let box = $state<VoicemailBox | null>(null);
	let pin = $state('');
	let error = $state('');
	let saved = $state(false);
	let previewKey = $state(0);

	async function load() {
		try {
			box = await api.get<VoicemailBox>(`/extensions/${extensionId}/voicemail`);
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	// Refresh while a TTS greeting is being rendered.
	$effect(() => {
		if (box?.greeting_status !== 'pending') return;
		const timer = setTimeout(async () => {
			await load();
			previewKey++;
		}, 3000);
		return () => clearTimeout(timer);
	});

	function useTts() {
		if (!box) return;
		box.greeting = 'tts';
		if (!box.greeting_text)
			box.greeting_text = t('vm.greetingTemplate', { name: displayName || '…' });
	}

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!box) return;
		error = '';
		saved = false;
		try {
			box = await api.put<VoicemailBox>(`/extensions/${extensionId}/voicemail`, {
				enabled: box.enabled,
				pin: pin === '' ? null : pin,
				email_notify: box.email_notify,
				attach_audio: box.attach_audio,
				language: box.language || null,
				greeting: box.greeting,
				greeting_text: box.greeting_text,
				max_message_secs: Number(box.max_message_secs)
			});
			pin = '';
			saved = true;
			previewKey++;
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
			<label class="flex items-center gap-2">
				<input type="radio" value="default" bind:group={box.greeting} />
				{t('vm.greetingDefault')}
			</label>
			<label class="flex items-center gap-2">
				<input type="radio" value="tts" checked={box.greeting === 'tts'} onchange={useTts} />
				{t('vm.greetingTts')}
			</label>
			{#if box.greeting === 'tts'}
				<textarea class="input" rows="3" maxlength="1000" bind:value={box.greeting_text}></textarea>
			{/if}
			<label class="flex items-center gap-2">
				<input type="radio" value="recorded" bind:group={box.greeting} />
				{t('vm.greetingRecorded')}
			</label>
			<p class="hint">{t('vm.greetingRecordHint')}</p>
			{#if box.greeting !== 'default'}
				{#if box.greeting_status === 'pending'}
					<p class="text-sm text-slate-500">{t('vm.greetingPending')}</p>
				{:else if box.greeting_status === 'failed'}
					<p class="text-sm text-red-600">{t('vm.greetingFailed')}</p>
				{:else if box.greeting_status === 'ready'}
					{#key previewKey}
						<audio
							controls
							preload="none"
							src="/api/v1/extensions/{extensionId}/voicemail/greeting?v={previewKey}"
						></audio>
					{/key}
				{/if}
			{/if}
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
