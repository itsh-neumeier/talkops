<script lang="ts">
	import { isBlocked } from '#lib/calls/blocking.ts';
	import PageHeader from '#lib/components/PageHeader.svelte';
	import PhoneNumber from '#lib/components/PhoneNumber.svelte';
	import LoadingContent from '#lib/components/LoadingContent.svelte';
	import Skeleton from '#lib/components/Skeleton.svelte';
	import { onDestroy, onMount } from 'svelte';
	import { SessionState, Web, type Session } from 'sip.js';
	import { ApiError, api, type Call, type Contact } from '#lib/api.ts';
	import { page } from '$app/state';
	import { can, session } from '#lib/session.svelte.ts';
	import SoftphoneAudio from '#lib/components/SoftphoneAudio.svelte';
	import {
		callTarget,
		callbackNumber,
		missedIncoming,
		offersVideo,
		setAudioOutput,
		toneUrl
	} from '#lib/softphone.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
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
	type Choice = {
		extension_id: string;
		extension_number: string;
		display_name: string;
		outbound_number: string | null;
		uses_default_number: boolean;
		hide_caller_id: boolean;
	};
	let { full = true } = $props<{ full?: boolean }>();
	let choices = $state<Choice[]>([]);
	let selected = $state('');
	let switching = $state(false);
	let microphone = $state('');
	let output = $state('');
	let ringtone = $state(true);
	let soundBlocked = $state(false);
	let notifications = $state(false);
	let notificationSupported = $state(false);
	let preferencesLoaded = $state(false);
	let calls = $state<Call[]>([]);
	let historyLoading = $state(true);
	let historyOwner: string | undefined;
	let historyGeneration = 0;
	let contacts = $state<Contact[]>([]);
	let contactSearch = $state('');
	let favorites = $state<string[]>([]);
	let historyFilter = $state<'all' | 'missed'>('all');
	let dataError = $state('');
	let disposed = false;
	let accountGeneration = 0;
	let historyTimer: ReturnType<typeof setTimeout> | undefined;
	let soundTimer: ReturnType<typeof setTimeout> | undefined;
	let incomingNotification: Notification | undefined;
	let ringAudio = $state<HTMLAudioElement>();
	const preferenceKey = `talkops.softphone.${session.user?.id}`;
	const currentChoice = $derived(choices.find((c) => c.extension_id === selected));
	const filteredContacts = $derived(
		contacts
			.filter((c) =>
				[c.name, c.company, c.phone_work, c.phone_mobile, c.phone_other].some((v) =>
					v.toLowerCase().includes(contactSearch.trim().toLowerCase())
				)
			)
			.sort(
				(a, b) =>
					Number(favorites.includes(b.id)) - Number(favorites.includes(a.id)) ||
					a.name.localeCompare(b.name)
			)
	);
	const recentCalls = $derived(
		calls.filter((c) => historyFilter === 'all' || missedIncoming(c, selected))
	);

	let account = $state<Account | null>(null);
	let status = $state<State>('offline');
	let registered = false;
	let number = $state('');
	let video = $state(false);
	/** The ringing call offers video (TalkOps allows video for it). */
	let videoOffered = $state(false);
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

	/** State of the current call in sip.js (`undefined` without a call). */
	function sessionState(): SessionState | undefined {
		return (user as unknown as { session?: Session } | null)?.session?.state;
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

	async function loadChoices() {
		error = '';
		try {
			choices = await api.get<Choice[]>('/me/webrtc/extensions');
			if (disposed) return;
			noExtension = choices.length === 0;
			if (selected && !choices.some((c) => c.extension_id === selected) && !busy) {
				selected = '';
				await stopUser();
			}
			if (!can('softphone.any_extension') && !selected && choices.length) {
				selected = choices[0].extension_id;
				await start();
			}
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function loadData() {
		const id = selected;
		const run = ++historyGeneration;
		if (historyOwner !== id) {
			historyLoading = true;
			calls = [];
		}
		dataError = '';
		const results = await Promise.allSettled([
			api.get<Contact[]>('/contacts'),
			id
				? api.get<Call[]>(`/me/webrtc/calls?extension_id=${encodeURIComponent(id)}`)
				: Promise.resolve([])
		]);
		if (disposed || id !== selected || run !== historyGeneration) return;
		historyOwner = id;
		historyLoading = false;
		if (results[0].status === 'fulfilled') contacts = results[0].value;
		else dataError = errorMessage(results[0].reason);
		if (results[1].status === 'fulfilled') calls = results[1].value;
		else dataError = errorMessage(results[1].reason);
	}

	async function refreshData() {
		if (secure) await loadChoices();
		await loadData();
	}

	function savePreferences() {
		if (!preferencesLoaded) return;
		try {
			localStorage.setItem(
				preferenceKey,
				JSON.stringify({ microphone, output, ringtone, notifications, favorites })
			);
		} catch {
			/* Storage can be disabled. */
		}
	}

	function favorite(id: string) {
		favorites = favorites.includes(id) ? favorites.filter((f) => f !== id) : [...favorites, id];
		savePreferences();
	}

	function chooseNumber(value: string) {
		if (!busy) number = value;
	}

	function dialNumber(value: string) {
		if (status !== 'ready' || switching) return;
		number = value;
		void call(false);
	}

	async function enableSound(test = false) {
		try {
			if (!ringAudio) return;
			clearTimeout(soundTimer);
			ringAudio.volume = test || status === 'ringing' ? 1 : 0;
			await setAudioOutput(ringAudio, output);
			// A user gesture permits later incoming-call playback in this tab.
			await ringAudio.play();
			soundBlocked = false;
			if (status !== 'ringing') {
				const stop = () => {
					if (ringAudio && status !== 'ringing') {
						ringAudio.pause();
						ringAudio.currentTime = 0;
					}
				};
				if (test) soundTimer = setTimeout(stop, 600);
				else stop();
			}
		} catch {
			soundBlocked = true;
		}
	}

	async function enableNotifications() {
		if (!notificationSupported) return;
		if (notifications) notifications = false;
		else notifications = (await Notification.requestPermission()) === 'granted';
		savePreferences();
	}

	async function changeIdentity() {
		if (busy || switching) return;
		void enableSound();
		await start();
	}

	async function stopUser() {
		const old = user;
		user = null;
		account = null;
		registered = false;
		status = 'offline';
		if (old) {
			try {
				await old.unregister();
			} catch {
				/* The transport can already be closed. */
			}
			await old.disconnect().catch(() => {});
		}
	}

	async function start() {
		if (!selected || switching || busy || disposed) return;
		switching = true;
		const generation = ++accountGeneration;
		calls = [];
		historyOwner = undefined;
		void loadData();
		lastDialed = '';
		await stopUser();
		if (disposed) {
			switching = false;
			return;
		}
		error = '';
		try {
			const loaded = await api.post<Account>('/me/webrtc', { extension_id: selected });
			if (disposed || generation !== accountGeneration) {
				switching = false;
				return;
			}
			account = loaded;
		} catch (err) {
			error =
				err instanceof ApiError && err.status === 404
					? t('phone.identityUnavailable')
					: errorMessage(err);
			switching = false;
			return;
		}
		const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
		status = 'connecting';
		user = new Web.SimpleUser(`${scheme}://${location.host}${account.ws_path}`, {
			reconnectionAttempts: 10,
			reconnectionDelay: 4,
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
				onRegistered: () => {
					if (generation !== accountGeneration || disposed) return;
					registered = true;
					if (!busy) status = 'ready';
					error = '';
				},
				onUnregistered: () => {
					if (generation !== accountGeneration || disposed) return;
					registered = false;
					if (!busy) status = 'offline';
				},
				onServerDisconnect: () => {
					if (generation !== accountGeneration || disposed) return;
					registered = false;
					if (!busy) status = 'offline';
					error = t('phone.disconnected');
				},
				onCallCreated: () => {
					status = 'calling';
					readPeer();
				},
				onCallReceived: () => {
					status = 'ringing';
					readPeer();
					videoOffered = offersVideo(
						(user as unknown as { session?: { request?: { body?: string } } } | null)?.session
							?.request?.body
					);
					if (notifications && notificationSupported && Notification.permission === 'granted') {
						try {
							incomingNotification = new Notification(t('phone.incoming'), {
								body: peer?.name || peer?.number || t('phone.unknown'),
								tag: 'talkops-incoming'
							});
							incomingNotification.onclick = () => {
								window.focus();
								incomingNotification?.close();
							};
						} catch {
							/* Notifications may be unavailable in this browser. */
						}
					}
				},
				onCallAnswered: () => {
					status = 'incall';
					muted = false;
					held = false;
					connectedAt = Date.now();
				},
				onCallHangup: () => {
					if (generation !== accountGeneration || disposed) return;
					status = registered ? 'ready' : 'offline';
					muted = false;
					held = false;
					peer = null;
					connectedAt = 0;
					number = '';
					confOpen = false;
					confNumber = '';
					conferenced = [];
					clearTimeout(historyTimer);
					historyTimer = setTimeout(() => {
						if (!disposed) void loadData();
					}, 1500);
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
		} finally {
			switching = false;
		}
	}

	onMount(() => {
		// Prefill the number from a link: `/phone?dial=200` or `/phone?dial=name@example.org`.
		const dial = new URLSearchParams(location.search).get('dial');
		if (dial) number = dial.slice(0, 320);
		try {
			const saved = JSON.parse(localStorage.getItem(preferenceKey) || '{}');
			microphone = typeof saved.microphone === 'string' ? saved.microphone : '';
			output = typeof saved.output === 'string' ? saved.output : '';
			ringtone = saved.ringtone !== false;
			notifications = saved.notifications === true;
			favorites = Array.isArray(saved.favorites)
				? saved.favorites.filter((id: unknown) => typeof id === 'string')
				: [];
		} catch {
			/* Use defaults if storage is unavailable. */
		}
		notificationSupported = 'Notification' in window;
		preferencesLoaded = true;
		ringAudio = new Audio(toneUrl(true));
		ringAudio.loop = true;
		soundBlocked = true;
		if (secure) void loadChoices();
		void loadData();
	});
	onDestroy(() => {
		disposed = true;
		accountGeneration++;
		clearTimeout(historyTimer);
		clearTimeout(soundTimer);
		clearTimeout(flashTimer);
		incomingNotification?.close();
		ringAudio?.pause();
		if (ringAudio) URL.revokeObjectURL(ringAudio.src);
		const u = user;
		user = null;
		u?.unregister()
			.catch(() => {})
			.finally(() => u.disconnect().catch(() => {}));
	});

	/** Hide the own number for the next call only (`*31` in front). */
	let hideOnce = $state(false);

	async function call(withVideo: boolean) {
		const target = callTarget(number || lastDialed);
		if (!user || !target || !account || status !== 'ready') return;
		void enableSound();
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
				sessionDescriptionHandlerOptions: {
					constraints: {
						audio: microphone ? { deviceId: { exact: microphone } } : true,
						video: withVideo
					}
				}
			});
		} catch (err) {
			status = 'ready';
			error = errorMessage(err);
		}
	}

	/**
	 * Accepts a ringing call once: a second accept (button and Enter at the
	 * same time, a double click) would fail with "Invalid session state".
	 */
	let answering = false;
	async function answer(withVideo: boolean) {
		if (answering || status !== 'ringing' || sessionState() !== SessionState.Initial) return;
		answering = true;
		video = withVideo;
		try {
			await user?.answer({
				sessionDescriptionHandlerOptions: {
					constraints: {
						audio: microphone ? { deviceId: { exact: microphone } } : true,
						video: withVideo
					}
				}
			});
		} catch (err) {
			error = errorMessage(err);
		} finally {
			answering = false;
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
		if (!full) return;
		if (e.ctrlKey || e.metaKey || e.altKey) return;
		const target = e.target as HTMLElement | null;
		const inNumber = target?.id === 'phone-number';
		if (!inNumber && target?.closest('input, textarea, select, [contenteditable]')) return;
		// Enter on a focused button already clicks it.
		if (e.key === 'Enter' && target?.closest('button')) return;
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
		// Hold is a re-INVITE: only possible once the call is fully set up.
		if (sessionState() !== SessionState.Established) return;
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
	$effect(() => {
		if (!ringAudio) return;
		if (status === 'ringing' && ringtone) {
			ringAudio.volume = 1;
			void ringAudio
				.play()
				.then(() => {
					soundBlocked = false;
				})
				.catch(() => {
					soundBlocked = true;
				});
		} else {
			ringAudio.pause();
			ringAudio.currentTime = 0;
		}
		if (status !== 'ringing') incomingNotification?.close();
	});
	$effect(() => {
		if (remoteAudio)
			void setAudioOutput(remoteAudio, output).catch((err) => {
				error = errorMessage(err);
			});
		if (remoteVideo)
			void setAudioOutput(remoteVideo, output).catch((err) => {
				error = errorMessage(err);
			});
		if (ringAudio)
			void setAudioOutput(ringAudio, output).catch((err) => {
				error = errorMessage(err);
			});
	});
	$effect(() => {
		void microphone;
		void output;
		void ringtone;
		void notifications;
		savePreferences();
	});
	$effect(() => {
		const dial = page.url.pathname === '/phone' ? page.url.searchParams.get('dial') : null;
		if (dial && !busy) number = dial.slice(0, 320);
	});
	const badge = $derived(
		status === 'ready' || status === 'incall'
			? 'badge-ok'
			: status === 'offline'
				? 'badge-bad'
				: 'badge-muted'
	);
</script>

<svelte:window onkeydown={onKey} />

<div class="mx-auto max-w-3xl space-y-4" class:hidden={!full}>
	<PageHeader title={t('nav.phone')} actions={account ? accountInfo : undefined} />
	{#snippet accountInfo()}
		<span class="text-sm text-slate-500 dark:text-slate-400"
			><span class="font-mono">{account!.extension_number}</span> · {account!.display_name}
			<span class="badge {badge}">{t(`phone.state.${status}`)}</span></span
		>
	{/snippet}
	{#if !secure}
		<div class="card text-sm">{t('phone.insecure')}</div>
	{/if}
	<ErrorBox {error} />
	{#if noExtension}<p class="text-sm text-slate-500">{t('phone.noIdentity')}</p>{/if}
	{#if choices.length}
		<div class="card space-y-2">
			{#if can('softphone.any_extension') || choices.length > 1}
				<label for="phone-identity">{t('phone.identity')}</label>
				<select
					id="phone-identity"
					class="input"
					bind:value={selected}
					disabled={busy || switching}
					onchange={changeIdentity}
				>
					<option value="" disabled>{t('phone.selectIdentity')}</option>
					{#each choices as choice (choice.extension_id)}
						<option value={choice.extension_id}
							>{choice.extension_number} · {choice.display_name} · {choice.outbound_number ??
								t('phone.noOutbound')}{choice.uses_default_number && choice.outbound_number
								? ` (${t('ext.defaultNumber')})`
								: ''}</option
						>
					{/each}
				</select>
			{:else}<p>{choices[0].extension_number} · {choices[0].display_name}</p>{/if}
			{#if currentChoice}<p class="text-sm">
					{t('phone.outbound')}:
					<span class="font-mono">{currentChoice.outbound_number ?? t('phone.noOutbound')}</span
					>{#if currentChoice.hide_caller_id}
						· {t('ext.hideCallerId')}{/if}
				</p>{/if}
			<p class="hint">{t('phone.identityHelp')}</p>
			{#if selected && status === 'offline'}<button
					class="btn btn-sm"
					disabled={switching}
					onclick={start}>{t('phone.reconnect')}</button
				>{/if}
		</div>
	{/if}

	<div class="grid grid-cols-1 gap-4 @2xl:grid-cols-[1fr_16rem]">
		<div
			class="card relative aspect-video overflow-hidden p-0 {busy
				? 'bg-slate-900'
				: 'bg-slate-100 dark:bg-slate-900'}"
		>
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
						{#if account?.video_enabled && videoOffered}
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
				<div
					class="absolute inset-0 flex items-center justify-center text-sm text-slate-500 dark:text-slate-400"
				>
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
	{#if secure}
		<SoftphoneAudio bind:microphone bind:output disabled={busy || switching} active={full} />
		<div class="card flex flex-wrap items-center gap-3">
			<label class="flex items-center gap-2"
				><input type="checkbox" bind:checked={ringtone} />{t('phone.ringtone')}</label
			>
			{#if ringtone && soundBlocked}<button class="btn btn-sm" onclick={() => enableSound(true)}
					>{t('phone.enableSound')}</button
				>{/if}
			{#if notificationSupported}<button
					class="btn btn-sm"
					aria-pressed={notifications}
					onclick={enableNotifications}
					>{notifications
						? t('phone.disableNotifications')
						: t('phone.enableNotifications')}</button
				>{/if}
		</div>
	{/if}
	<ErrorBox error={dataError} />
	<div class="grid grid-cols-1 gap-4 @3xl:grid-cols-2">
		<section class="card space-y-3" aria-label={t('nav.phonebook')}>
			<div class="flex items-center justify-between">
				<h2>{t('nav.phonebook')}</h2>
				<button class="btn btn-sm" disabled={switching} onclick={refreshData}
					>{t('phone.refresh')}</button
				>
			</div>
			<input
				class="input"
				aria-label={t('phone.searchContacts')}
				placeholder={t('phone.searchContacts')}
				bind:value={contactSearch}
			/>
			<div class="max-h-80 space-y-3 overflow-y-auto">
				{#each filteredContacts as contact (contact.id)}
					<div>
						<div class="flex items-center gap-2">
							<button
								class="btn btn-sm"
								aria-label={t('phone.favorite', { name: contact.name })}
								aria-pressed={favorites.includes(contact.id)}
								onclick={() => favorite(contact.id)}
								>{favorites.includes(contact.id) ? '★' : '☆'}</button
							><span>{contact.name}{contact.company ? ` · ${contact.company}` : ''}</span>
						</div>
						<div class="flex flex-wrap gap-1 mt-1">
							{#each [...new Set([contact.phone_work, contact.phone_mobile, contact.phone_other].filter(Boolean))] as phone (phone)}<button
									class="btn btn-sm font-mono"
									disabled={busy}
									onclick={() => chooseNumber(phone)}>{phone}</button
								><button
									class="btn btn-sm"
									aria-label={t('phone.callNumber', { number: phone })}
									disabled={status !== 'ready'}
									onclick={() => dialNumber(phone)}>📞</button
								>{/each}
						</div>
					</div>
				{:else}<p class="hint">{t('contacts.none')}</p>{/each}
			</div>
		</section>
		<section class="card space-y-3" aria-label={t('phone.recentCalls')}>
			<div class="flex flex-wrap items-center justify-between gap-2">
				<h2>{t('phone.recentCalls')}</h2>
				<select
					class="input mt-0 w-auto"
					aria-label={t('phone.callFilter')}
					bind:value={historyFilter}
					><option value="all">{t('phone.allCalls')}</option><option value="missed"
						>{t('phone.missedCalls')}</option
					></select
				>
			</div>
			<LoadingContent pending={historyLoading}>
				{#snippet placeholder()}<Skeleton lines={6} />{/snippet}
				<div class="content-in max-h-80 space-y-2 overflow-y-auto">
					{#each recentCalls as c (c.id)}
						<div class="border-b border-slate-200 pb-2 dark:border-slate-700">
							<button
								class="btn btn-sm font-mono"
								aria-label={callbackNumber(c, selected)}
								disabled={busy}
								onclick={() => chooseNumber(callbackNumber(c, selected))}
								><PhoneNumber
									number={callbackNumber(c, selected)}
									countryNumber={(c.direction === 'inbound' ? c.caller : c.called)?.number ||
										callbackNumber(c, selected)}
									flag={c.direction === 'inbound' || c.direction === 'outbound'}
									internal={!!(c.direction === 'inbound' ? c.caller : c.called)?.extension_id}
								/></button
							>
							<span class="ml-1 text-sm"
								><button
									class="btn btn-sm"
									aria-label={t('phone.callNumber', { number: callbackNumber(c, selected) })}
									disabled={status !== 'ready'}
									onclick={() => dialNumber(callbackNumber(c, selected))}>📞</button
								>
								{t(`dir.${c.direction}`)}{#if isBlocked(c)}
									· {t('calls.result.blocked')}{:else if missedIncoming(c, selected)}
									· {t('calls.missed')}{:else if !c.answered_at}
									· {t('phone.unanswered')}{/if}</span
							>
							<p class="hint">{formatDateTime(c.started_at)}</p>
						</div>
					{:else}{#if !dataError}<p class="hint">
								{selected ? t('calls.empty') : t('phone.selectIdentity')}
							</p>{/if}{/each}
				</div>
			</LoadingContent>
		</section>
	</div>
</div>

{#if !full && account}
	<aside
		class="card fixed right-4 bottom-4 z-40 max-w-sm space-y-2 shadow-lg"
		aria-label={t('nav.phone')}
	>
		<a href="/phone" class="font-medium">{t('nav.phone')} · {account.extension_number}</a>
		<span class="badge {badge}">{t(`phone.state.${status}`)}</span>
		{#if peer}<p>{peer.name || peer.number} {status === 'incall' ? duration : ''}</p>{/if}
		{#if error}<p role="alert" class="text-sm text-red-600">{error}</p>{/if}
		<div class="flex gap-2">
			{#if status === 'ringing'}<button class="btn btn-primary" onclick={() => answer(false)}
					>{t('phone.answer')}</button
				><button class="btn btn-danger" onclick={() => user?.decline()}>{t('phone.decline')}</button
				>
			{:else if busy}<button class="btn" aria-pressed={muted} onclick={toggleMute}
					>{muted ? t('phone.unmute') : t('phone.mute')}</button
				><button class="btn btn-danger" onclick={() => user?.hangup()}>{t('phone.hangup')}</button>
			{:else if status === 'offline'}<button class="btn" disabled={switching} onclick={start}
					>{t('phone.reconnect')}</button
				>{/if}
		</div>
	</aside>
{/if}
