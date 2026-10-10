<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { Web, type Session } from 'sip.js';
	import { ApiError, api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	type Account = {
		extension_id: string;
		extension_number: string;
		display_name: string;
		sip_username: string;
		sip_password: string;
		sip_domain: string;
		ws_path: string;
		/** TURN relays (with short-lived credentials), if configured. */
		ice_servers: RTCIceServer[];
		relay_only: boolean;
		video_enabled: boolean;
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
	/** The other party of the current call. */
	let peer = $state<{ name: string; number: string } | null>(null);
	/** For redialling: an empty number calls the last one again. */
	let lastDialed = $state('');
	let connectedAt = $state(0);
	/** Conference: the form to add someone, and who was added. */
	let confOpen = $state(false);
	let confNumber = $state('');
	let confBusy = $state(false);
	let conferenced = $state<string[]>([]);
	let now = $state(Date.now());
	let localVideo = $state<HTMLVideoElement>();
	let remoteVideo = $state<HTMLVideoElement>();
	let remoteAudio = $state<HTMLAudioElement>();
	let user: Web.SimpleUser | null = null;
	const secure = typeof window !== 'undefined' && window.isSecureContext;

	const keys = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '*', '0', '#'];

	/** Caller or callee as FreeSWITCH presents it (device logins `20-1` → `20`). */
	function readPeer() {
		const session = (user as unknown as { session?: Session } | null)?.session;
		const id = session?.remoteIdentity;
		if (!id) return;
		const raw = id.uri.user ?? '';
		const number = raw.replace(/^(\d+)-\d+$/, '$1');
		const name = id.displayName && id.displayName !== raw ? id.displayName : '';
		peer = { name, number };
	}

	function sipDebug(): boolean {
		try {
			return localStorage.getItem('talkops.sipDebug') === '1';
		} catch {
			return false;
		}
	}

	/**
	 * Key tones as RTP telephone events (RFC 2833, what voicemail and menus
	 * expect); SIP INFO only if the browser cannot send them.
	 */
	function sendTone(k: string) {
		const session = (user as unknown as { session?: Session } | null)?.session;
		const sdh = session?.sessionDescriptionHandler as Web.SessionDescriptionHandler | undefined;
		let sent = false;
		try {
			sent = sdh?.sendDtmf(k, { duration: 120, interToneGap: 70 }) ?? false;
		} catch {
			sent = false;
		}
		if (!sent) user?.sendDTMF(k).catch(() => {});
	}

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
				// `localStorage['talkops.sipDebug'] = '1'` logs SIP to the console.
				logBuiltinEnabled: sipDebug(),
				logLevel: sipDebug() ? 'debug' : 'error',
				sessionDescriptionHandlerFactoryOptions: {
					// Without TURN no external STUN server is asked: TalkOps finds
					// the browser's address itself.
					peerConnectionConfiguration: {
						iceServers: account.ice_servers,
						iceTransportPolicy: account.relay_only ? 'relay' : 'all'
					},
					// TURN over TCP takes a moment to allocate.
					iceGatheringTimeout: account.ice_servers.length ? 3000 : 500
				}
			},
			delegate: {
				onRegistered: () => (status = 'ready'),
				onUnregistered: () => (status = 'offline'),
				onServerDisconnect: () => {
					status = 'offline';
					error = t('phone.disconnected');
				},
				onCallCreated: () => {
					status = 'calling';
					readPeer();
				},
				onCallReceived: () => {
					status = 'ringing';
					readPeer();
				},
				onCallAnswered: () => {
					status = 'incall';
					muted = false;
					held = false;
					connectedAt = Date.now();
				},
				onCallHangup: () => {
					status = user ? 'ready' : 'offline';
					muted = false;
					held = false;
					peer = null;
					connectedAt = 0;
					number = '';
					confOpen = false;
					confNumber = '';
					conferenced = [];
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
		// Test calls from the diagnostics page: `/phone?dial=test.echo@sip5060.net`.
		const dial = new URLSearchParams(location.search).get('dial');
		if (dial) number = dial.slice(0, 320);
		if (secure) start();
	});
	onDestroy(() => {
		const u = user;
		user = null;
		u?.unregister()
			.catch(() => {})
			.finally(() => u.disconnect().catch(() => {}));
	});

	/** A number, or a SIP address (`name@domain`) called as is. */
	function callTarget(input: string): { display: string; uri: string | null } | null {
		const raw = input.trim().replace(/^sip:/i, '');
		const addr = raw.match(/^([A-Za-z0-9._~!*'()+-]{1,64})@([A-Za-z0-9.-]{3,253})$/);
		if (addr) return { display: raw, uri: `sip:${addr[1]}@${addr[2].toLowerCase()}` };
		const digits = raw.replace(/[^0-9*#+]/g, '');
		return digits ? { display: digits, uri: null } : null;
	}

	/** Hide the own number for the next call only (`*31` in front). */
	let hideOnce = $state(false);

	async function call(withVideo: boolean) {
		const target = callTarget(number || lastDialed);
		if (!user || !target || !account) return;
		const dest = target.display;
		error = '';
		lastDialed = dest;
		peer = { name: '', number: dest };
		video = withVideo;
		const prefix = hideOnce ? '*31' : '';
		hideOnce = false;
		const uri = target.uri
			? target.uri.replace(/^sip:/, `sip:${prefix}`)
			: `sip:${prefix}${dest}@${account.sip_domain}`;
		try {
			await user.call(uri, {
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

	// The key last pressed (highlighted briefly on the keypad).
	let flashed = $state('');
	let flashTimer: ReturnType<typeof setTimeout> | undefined;

	function press(k: string) {
		flashed = k;
		clearTimeout(flashTimer);
		flashTimer = setTimeout(() => (flashed = ''), 150);
		if (status === 'incall') sendTone(k);
		else number += k;
	}

	/**
	 * Keyboard and numpad: digits, * and # dial (DTMF during a call),
	 * Backspace deletes, Enter calls or answers, Escape hangs up, declines
	 * or clears. Typing into another field is left alone.
	 */
	function onKey(e: KeyboardEvent) {
		if (e.ctrlKey || e.metaKey || e.altKey) return;
		const target = e.target as HTMLElement | null;
		const inNumber = target?.id === 'phone-number';
		if (!inNumber && target?.closest('input, textarea, select, [contenteditable]')) return;
		const key = e.key === 'Multiply' ? '*' : e.key;
		if (/^[0-9*#]$/.test(key) || (key === '+' && status !== 'incall')) {
			if (inNumber && status !== 'incall') return; // the field types it itself
			e.preventDefault();
			press(key);
		} else if (key === 'Backspace' && !inNumber && status !== 'incall') {
			e.preventDefault();
			number = number.slice(0, -1);
		} else if (key === 'Enter') {
			e.preventDefault();
			if (status === 'ringing') answer(false);
			else if (status === 'ready') call(false);
		} else if (key === 'Escape') {
			e.preventDefault();
			if (status === 'ringing') user?.decline();
			else if (busy) user?.hangup();
			else number = '';
		}
	}

	function toggleMute() {
		if (!user) return;
		if (muted) user.unmute();
		else user.mute();
		muted = !muted;
	}

	/** Adds a participant: the call becomes a conference on the server. */
	async function addParticipant(e: SubmitEvent) {
		e.preventDefault();
		const session = (user as unknown as { session?: Session } | null)?.session;
		const callId = session?.dialog?.callId;
		const dest = confNumber.replace(/[^0-9*#+]/g, '');
		if (!account || !callId || !dest) return;
		error = '';
		confBusy = true;
		try {
			await api.post('/me/webrtc/conference', {
				extension_id: account.extension_id,
				call_id: callId,
				number: dest
			});
			conferenced = [...conferenced, dest];
			confNumber = '';
			confOpen = false;
		} catch (err) {
			error = errorMessage(err);
		} finally {
			confBusy = false;
		}
	}

	async function toggleHold() {
		try {
			if (held) await user?.unhold();
			else await user?.hold();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	// Call duration.
	$effect(() => {
		if (!connectedAt) return;
		const timer = setInterval(() => (now = Date.now()), 1000);
		return () => clearInterval(timer);
	});
	const duration = $derived.by(() => {
		const secs = connectedAt ? Math.max(0, Math.floor((now - connectedAt) / 1000)) : 0;
		const mm = String(Math.floor(secs / 60)).padStart(2, '0');
		return `${mm}:${String(secs % 60).padStart(2, '0')}`;
	});

	const busy = $derived(status === 'calling' || status === 'incall' || status === 'ringing');
	const badge = $derived(
		status === 'ready' || status === 'incall'
			? 'badge-ok'
			: status === 'offline'
				? 'badge-bad'
				: 'badge-muted'
	);
</script>

<svelte:window onkeydown={onKey} />

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
					<p class="text-sm text-slate-300">📞 {t('phone.incoming')}</p>
					{#if peer}
						<p class="text-center">
							<span class="block text-2xl font-semibold" data-testid="peer-name"
								>{peer.name || peer.number || t('phone.unknown')}</span
							>
							{#if peer.name && peer.number}<span
									class="block font-mono text-slate-300"
									data-testid="peer-number">{peer.number}</span
								>{/if}
						</p>
					{/if}
					<div class="flex flex-wrap justify-center gap-2">
						<button class="btn btn-primary" onclick={() => answer(false)}
							>{t('phone.answer')}</button
						>
						{#if account?.video_enabled}
							<button class="btn btn-primary" onclick={() => answer(true)}
								>{t('phone.answerVideo')}</button
							>
						{/if}
						<button class="btn btn-danger" onclick={() => user?.decline()}
							>{t('phone.decline')}</button
						>
					</div>
				</div>
			{:else if busy && peer}
				<div
					class="absolute top-2 left-2 rounded-lg bg-slate-900/70 px-3 py-1.5 text-sm text-white"
					data-testid="peer"
				>
					<span class="font-medium">{peer.name || peer.number || t('phone.unknown')}</span>
					{#if peer.name && peer.number}<span class="ml-1 font-mono text-slate-300"
							>{peer.number}</span
						>{/if}
					<span class="ml-2 font-mono text-slate-300"
						>{status === 'incall' ? duration : t('phone.state.calling')}</span
					>
					{#if held}<span class="ml-2 text-amber-300">{t('phone.onHold')}</span>{/if}
				</div>
			{:else if !busy}
				<div class="absolute inset-0 flex items-center justify-center text-sm text-slate-400">
					{t('phone.idle')}
				</div>
			{/if}
		</div>

		<div class="card space-y-3">
			<input
				id="phone-number"
				class="input mt-0 text-center font-mono text-lg"
				bind:value={number}
				placeholder={lastDialed && !number ? lastDialed : t('phone.number')}
				aria-label={t('phone.number')}
				disabled={status === 'incall'}
			/>
			<div class="grid grid-cols-3 gap-2">
				{#each keys as k (k)}
					<button
						class="btn font-mono text-lg {flashed === k
							? 'bg-teal-600 text-white dark:bg-teal-600'
							: ''}"
						onclick={() => press(k)}>{k}</button
					>
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
				{#if status === 'incall'}
					{#if confOpen}
						<form class="flex gap-2" onsubmit={addParticipant}>
							<input
								class="input mt-0 flex-1 font-mono"
								bind:value={confNumber}
								placeholder={t('phone.number')}
								aria-label={t('phone.confNumber')}
							/>
							<button class="btn btn-primary" disabled={confBusy || !confNumber}
								>{t('phone.confAdd')}</button
							>
						</form>
					{:else}
						<button class="btn w-full" onclick={() => (confOpen = true)}
							>👥 {t('phone.conference')}</button
						>
					{/if}
					{#if conferenced.length}
						<p class="hint" data-testid="conference">
							{t('phone.confMembers', { numbers: conferenced.join(', ') })}
						</p>
					{/if}
				{/if}
				<button class="btn btn-danger w-full" onclick={() => user?.hangup()}
					>{t('phone.hangup')}</button
				>
			{:else if status !== 'ringing'}
				<div class="grid gap-2 {account?.video_enabled ? 'grid-cols-2' : 'grid-cols-1'}">
					<button class="btn btn-primary" disabled={status !== 'ready'} onclick={() => call(false)}
						>📞 {t('phone.call')}</button
					>
					{#if account?.video_enabled}
						<button class="btn btn-primary" disabled={status !== 'ready'} onclick={() => call(true)}
							>🎥 {t('phone.videoCall')}</button
						>
					{/if}
				</div>
				<label class="flex items-center gap-2 text-sm"
					><input type="checkbox" bind:checked={hideOnce} data-testid="hide-once" />
					{t('phone.hideOnce')}</label
				>
				{#if number}
					<button class="btn btn-sm w-full" onclick={() => (number = number.slice(0, -1))}>⌫</button
					>
				{/if}
			{/if}
		</div>
	</div>
	<p class="hint">{t('phone.hint')} {t('phone.keyboardHint')}</p>
</div>
