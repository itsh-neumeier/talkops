<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { Web } from 'sip.js';
	import { ApiError, api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	type Account = {
		extension_number: string;
		display_name: string;
		sip_username: string;
		sip_password: string;
		sip_domain: string;
		ws_path: string;
	};
	type State = 'offline' | 'connecting' | 'ready' | 'calling' | 'ringing' | 'incall';

	let account = $state<Account | null>(null);
	let status = $state<State>('offline');
	let number = $state('');
	let video = $state(false);
	let muted = $state(false);
	let held = $state(false);
	let error = $state('');
	let noExtension = $state(false);
	let localVideo = $state<HTMLVideoElement>();
	let remoteVideo = $state<HTMLVideoElement>();
	let remoteAudio = $state<HTMLAudioElement>();
	let user: Web.SimpleUser | null = null;
	const secure = typeof window !== 'undefined' && window.isSecureContext;

	const keys = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '*', '0', '#'];

	function media(withVideo: boolean) {
		return {
			constraints: { audio: true, video: withVideo },
			local: { video: localVideo },
			remote: { audio: remoteAudio, video: remoteVideo }
		};
	}

	async function start() {
		error = '';
		try {
			account = await api.post<Account>('/me/webrtc', {});
		} catch (err) {
			// 404: no extension assigned yet – the hint below says so.
			if (err instanceof ApiError && err.status === 404) noExtension = true;
			else error = errorMessage(err);
			return;
		}
		const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
		status = 'connecting';
		user = new Web.SimpleUser(`${scheme}://${location.host}${account.ws_path}`, {
			aor: `sip:${account.sip_username}@${account.sip_domain}`,
			media: media(false),
			userAgentOptions: {
				authorizationUsername: account.sip_username,
				authorizationPassword: account.sip_password,
				displayName: account.display_name,
				logBuiltinEnabled: false
			},
			delegate: {
				onRegistered: () => (status = 'ready'),
				onUnregistered: () => (status = 'offline'),
				onServerDisconnect: () => {
					status = 'offline';
					error = t('phone.disconnected');
				},
				onCallCreated: () => (status = 'calling'),
				onCallReceived: () => (status = 'ringing'),
				onCallAnswered: () => {
					status = 'incall';
					muted = false;
					held = false;
				},
				onCallHangup: () => {
					status = user ? 'ready' : 'offline';
					muted = false;
					held = false;
				},
				onCallHold: (h) => (held = h)
			}
		});
		try {
			await user.connect();
			await user.register();
		} catch (err) {
			status = 'offline';
			error = errorMessage(err);
		}
	}

	onMount(() => {
		if (secure) start();
	});
	onDestroy(() => {
		const u = user;
		user = null;
		u?.unregister()
			.catch(() => {})
			.finally(() => u.disconnect().catch(() => {}));
	});

	async function call(withVideo: boolean) {
		const dest = number.replace(/[^0-9*#+]/g, '');
		if (!user || !dest || !account) return;
		error = '';
		video = withVideo;
		try {
			await user.call(`sip:${dest}@${account.sip_domain}`, {
				sessionDescriptionHandlerOptions: { constraints: { audio: true, video: withVideo } }
			});
		} catch (err) {
			status = 'ready';
			error = errorMessage(err);
		}
	}

	async function answer(withVideo: boolean) {
		video = withVideo;
		try {
			await user?.answer({
				sessionDescriptionHandlerOptions: { constraints: { audio: true, video: withVideo } }
			});
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function press(k: string) {
		if (status === 'incall') user?.sendDTMF(k).catch(() => {});
		else number += k;
	}

	function toggleMute() {
		if (!user) return;
		if (muted) user.unmute();
		else user.mute();
		muted = !muted;
	}

	async function toggleHold() {
		try {
			if (held) await user?.unhold();
			else await user?.hold();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	const busy = $derived(status === 'calling' || status === 'incall' || status === 'ringing');
	const badge = $derived(
		status === 'ready' || status === 'incall'
			? 'badge-ok'
			: status === 'offline'
				? 'badge-bad'
				: 'badge-muted'
	);
</script>

<div class="mx-auto max-w-3xl space-y-4">
	<div class="flex flex-wrap items-center justify-between gap-2">
		<h1>{t('nav.phone')}</h1>
		{#if account}
			<span class="text-sm text-slate-500"
				><span class="font-mono">{account.extension_number}</span> · {account.display_name}
				<span class="badge {badge}">{t(`phone.state.${status}`)}</span></span
			>
		{/if}
	</div>
	{#if !secure}
		<div class="card text-sm">{t('phone.insecure')}</div>
	{/if}
	<ErrorBox {error} />
	{#if noExtension}<p class="text-sm text-slate-500">{t('phone.noExtension')}</p>{/if}

	<div class="grid gap-4 md:grid-cols-[1fr_16rem]">
		<div class="card relative aspect-video overflow-hidden bg-slate-900 p-0">
			<!-- svelte-ignore a11y_media_has_caption -->
			<video bind:this={remoteVideo} class="h-full w-full object-contain" autoplay playsinline
			></video>
			<!-- svelte-ignore a11y_media_has_caption -->
			<video
				bind:this={localVideo}
				class="absolute right-2 bottom-2 w-1/4 rounded border border-slate-700 {video && busy
					? ''
					: 'hidden'}"
				autoplay
				playsinline
				muted
			></video>
			<audio bind:this={remoteAudio} autoplay></audio>
			{#if status === 'ringing'}
				<div
					class="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-slate-900/80 text-white"
				>
					<p class="text-lg">📞 {t('phone.incoming')}</p>
					<div class="flex flex-wrap justify-center gap-2">
						<button class="btn btn-primary" onclick={() => answer(false)}
							>{t('phone.answer')}</button
						>
						<button class="btn btn-primary" onclick={() => answer(true)}
							>{t('phone.answerVideo')}</button
						>
						<button class="btn btn-danger" onclick={() => user?.decline()}
							>{t('phone.decline')}</button
						>
					</div>
				</div>
			{:else if !busy}
				<div class="absolute inset-0 flex items-center justify-center text-sm text-slate-400">
					{t('phone.idle')}
				</div>
			{/if}
		</div>

		<div class="card space-y-3">
			<input
				class="input mt-0 text-center font-mono text-lg"
				bind:value={number}
				placeholder={t('phone.number')}
				aria-label={t('phone.number')}
				disabled={status === 'incall'}
				onkeydown={(e) => e.key === 'Enter' && !busy && call(false)}
			/>
			<div class="grid grid-cols-3 gap-2">
				{#each keys as k (k)}
					<button class="btn font-mono text-lg" onclick={() => press(k)}>{k}</button>
				{/each}
			</div>
			{#if busy && status !== 'ringing'}
				<div class="grid grid-cols-2 gap-2">
					<button class="btn" aria-pressed={muted} onclick={toggleMute}
						>{muted ? t('phone.unmute') : t('phone.mute')}</button
					>
					<button
						class="btn"
						aria-pressed={held}
						disabled={status !== 'incall'}
						onclick={toggleHold}>{held ? t('phone.resume') : t('phone.hold')}</button
					>
				</div>
				<button class="btn btn-danger w-full" onclick={() => user?.hangup()}
					>{t('phone.hangup')}</button
				>
			{:else if status !== 'ringing'}
				<div class="grid grid-cols-2 gap-2">
					<button class="btn btn-primary" disabled={status !== 'ready'} onclick={() => call(false)}
						>📞 {t('phone.call')}</button
					>
					<button class="btn btn-primary" disabled={status !== 'ready'} onclick={() => call(true)}
						>🎥 {t('phone.videoCall')}</button
					>
				</div>
				{#if number}
					<button class="btn btn-sm w-full" onclick={() => (number = number.slice(0, -1))}>⌫</button
					>
				{/if}
			{/if}
		</div>
	</div>
	<p class="hint">{t('phone.hint')}</p>
</div>
