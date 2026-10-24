<script lang="ts">
	// UniFi Network connection (ADR 0041). The API key is stored encrypted
	// and never shown again; an empty field keeps it. The voice SSID and the
	// write switch belong to the SSID changes of ADR 0042.
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import { writeErrorKey, type WifiSummary } from '#lib/unifiWrite.ts';
	import { errorMessage } from '#lib/util.ts';
	import type { UnifiConnection } from '#lib/wifiSurvey.ts';

	type View = { connection: UnifiConnection | null; has_api_key: boolean };

	let view = $state<View | null>(null);
	let form = $state({ url: '', site: 'default', cert_sha256: '', api_key: '' });
	let error = $state('');
	let notice = $state('');
	let busy = $state(false);
	let ssids = $state<WifiSummary[] | null>(null);
	let voice = $state({ wifi_id: '', write_enabled: false });

	function apply(v: View) {
		view = v;
		form = {
			url: v.connection?.url ?? '',
			site: v.connection?.site ?? 'default',
			cert_sha256: v.connection?.cert_sha256 ?? '',
			api_key: ''
		};
		voice = {
			wifi_id: v.connection?.voice_wifi_id ?? '',
			write_enabled: v.connection?.write_enabled ?? false
		};
	}

	onMount(async () => {
		try {
			apply(await api.get<View>('/unifi/connection'));
		} catch (err) {
			error = errorMessage(err);
		}
	});

	async function run(action: () => Promise<void>, done?: string) {
		busy = true;
		notice = '';
		try {
			await action();
			error = '';
			if (done) notice = done;
		} catch (err) {
			const key = writeErrorKey(err);
			error = key ? t(key as MessageKey) : errorMessage(err);
		} finally {
			busy = false;
		}
	}

	const save = (e: SubmitEvent) => {
		e.preventDefault();
		return run(async () => {
			apply(
				await api.put<View>('/unifi/connection', {
					...form,
					api_key: form.api_key.trim() || null
				})
			);
		}, t('common.saved'));
	};
	const setPhoneNetwork = (enabled: boolean) =>
		run(async () => {
			apply(await api.put<View>('/unifi/phone-network', { enabled }));
		}, t('common.saved'));
	const test = () =>
		run(async () => {
			apply(await api.post<View>('/unifi/connection/test'));
		});
	const readCertificate = () =>
		run(async () => {
			const res = await api.post<{ cert_sha256: string }>('/unifi/certificate', { url: form.url });
			form.cert_sha256 = res.cert_sha256;
		}, t('wifi.unifi.certRead'));
	const remove = () =>
		run(async () => {
			if (!confirm(t('wifi.unifi.confirmDelete'))) return;
			await api.del('/unifi/connection');
			apply({ connection: null, has_api_key: false });
		});

	const loadSsids = () =>
		run(async () => {
			ssids = await api.get<WifiSummary[]>('/unifi/wifi-broadcasts');
		});
	const saveVoice = (e: SubmitEvent) => {
		e.preventDefault();
		return run(async () => {
			apply(
				await api.put<View>('/unifi/voice-ssid', {
					wifi_id: voice.wifi_id || null,
					write_enabled: voice.write_enabled && !!voice.wifi_id
				})
			);
		}, t('common.saved'));
	};

	const caps = $derived(view?.connection?.capabilities ?? {});
	const versionOk = $derived(!!caps.write_supported);
	// Only standard, user-defined SSIDs can take writes.
	const writable = (w: WifiSummary) => w.type === 'STANDARD' && w.origin === 'USER_DEFINED';
	const chosen = $derived(ssids?.find((w) => w.id === voice.wifi_id));
	const canSwitch = $derived(
		can('unifi.write') && versionOk && !!voice.wifi_id && (!chosen || writable(chosen))
	);
</script>

<div class="space-y-4">
	<ErrorBox {error} />
	<form class="card space-y-3 p-4" onsubmit={save}>
		<h2>{t('wifi.unifi.title')}</h2>
		<p class="text-sm text-slate-500 dark:text-slate-400">{t('wifi.unifi.intro')}</p>
		<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2">
			<div>
				<label for="unifi-url">{t('wifi.unifi.url')}</label>
				<input
					id="unifi-url"
					class="input"
					type="url"
					required
					placeholder="https://192.168.1.1"
					bind:value={form.url}
				/>
			</div>
			<div>
				<label for="unifi-site">{t('wifi.unifi.site')}</label>
				<input id="unifi-site" class="input" required bind:value={form.site} />
			</div>
			<div class="@lg:col-span-2">
				<label for="unifi-key">{t('wifi.unifi.apiKey')}</label>
				<input
					id="unifi-key"
					class="input"
					type="password"
					autocomplete="off"
					placeholder={view?.has_api_key ? t('wifi.unifi.apiKeyKeep') : ''}
					required={!view?.has_api_key}
					bind:value={form.api_key}
				/>
				<p class="mt-1 text-xs text-slate-500 dark:text-slate-400">{t('wifi.unifi.apiKeyHint')}</p>
			</div>
			<div class="@lg:col-span-2">
				<label for="unifi-cert">{t('wifi.unifi.cert')}</label>
				<div class="flex gap-2">
					<input
						id="unifi-cert"
						class="input font-mono text-xs"
						placeholder={t('wifi.unifi.certPublic')}
						bind:value={form.cert_sha256}
					/>
					<button
						type="button"
						class="btn shrink-0"
						disabled={busy || !form.url.startsWith('https://')}
						onclick={readCertificate}>{t('wifi.unifi.certButton')}</button
					>
				</div>
				<p class="mt-1 text-xs text-slate-500 dark:text-slate-400">{t('wifi.unifi.certHint')}</p>
			</div>
		</div>
		<div class="flex flex-wrap items-center gap-2">
			<button class="btn btn-primary" disabled={busy}>{t('common.save')}</button>
			{#if view?.connection}
				<button type="button" class="btn" disabled={busy} onclick={test}
					>{t('wifi.unifi.test')}</button
				>
				<button type="button" class="btn btn-danger" disabled={busy} onclick={remove}
					>{t('common.delete')}</button
				>
			{/if}
			{#if notice}<span class="text-sm text-teal-700 dark:text-teal-400" role="status"
					>{notice}</span
				>{/if}
		</div>
	</form>

	{#if view?.connection}
		<section class="card space-y-2 p-4">
			<h2>{t('wifi.unifi.status')}</h2>
			<dl class="grid grid-cols-1 gap-x-6 gap-y-1 text-sm @lg:grid-cols-2">
				<dt class="text-slate-500">{t('wifi.unifi.lastCheck')}</dt>
				<dd>{formatDateTime(view.connection.last_check_at)}</dd>
				<dt class="text-slate-500">{t('wifi.unifi.version')}</dt>
				<dd>{view.connection.version || '—'}</dd>
				<dt class="text-slate-500">{t('wifi.unifi.integration')}</dt>
				<dd>
					<span class={caps.integration ? 'badge badge-ok' : 'badge badge-muted'}
						>{caps.integration ? t('wifi.unifi.available') : t('wifi.unifi.unavailable')}</span
					>
				</dd>
				<dt class="text-slate-500">{t('wifi.unifi.classic')}</dt>
				<dd>
					<span class={caps.classic ? 'badge badge-ok' : 'badge badge-warn'}
						>{caps.classic ? t('wifi.unifi.available') : t('wifi.unifi.unavailable')}</span
					>
				</dd>
				<dt class="text-slate-500">{t('wifi.unifi.accessPoints')}</dt>
				<dd>{caps.access_points ?? '—'}</dd>
				<dt class="text-slate-500">{t('unifiWrite.cap.wifiBroadcasts')}</dt>
				<dd>
					<span class={caps.wifi_broadcasts ? 'badge badge-ok' : 'badge badge-muted'}
						>{caps.wifi_broadcasts ? t('wifi.unifi.available') : t('wifi.unifi.unavailable')}</span
					>
				</dd>
				{#if caps.voice_wifi}
					<dt class="text-slate-500">{t('unifiWrite.cap.voiceWifi')}</dt>
					<dd>
						{caps.voice_wifi.found ? (caps.voice_wifi.name ?? '—') : t('unifiWrite.cap.notFound')}
						{#if caps.voice_wifi.not_writable}
							<span class="badge badge-warn"
								>{t(`unifiWrite.err.${caps.voice_wifi.not_writable}` as MessageKey)}</span
							>
						{/if}
					</dd>
					<dt class="text-slate-500">{t('unifiWrite.cap.fields')}</dt>
					<dd>
						{(caps.allowlist_fields ?? [])
							.map((f) => t(`unifiWrite.field.${f}` as MessageKey))
							.join(', ') || '—'}
					</dd>
				{/if}
				<dt class="text-slate-500">{t('unifiWrite.cap.write')}</dt>
				<dd>
					<span class={caps.write_supported ? 'badge badge-ok' : 'badge badge-muted'}
						>{caps.write_supported
							? t('unifiWrite.cap.possible')
							: t('unifiWrite.cap.notPossible')}</span
					>
				</dd>
			</dl>
			<p class="text-xs text-slate-500 dark:text-slate-400">{t('unifiWrite.cap.note')}</p>
			{#if view.connection.last_check_at && !caps.classic && !view.connection.last_error}
				<p class="text-sm text-amber-700 dark:text-amber-400">{t('wifi.unifi.noClassic')}</p>
			{/if}
			{#if view.connection.last_error}
				<p class="error-box">{view.connection.last_error}</p>
			{/if}
		</section>
	{/if}

	{#if view?.connection}
		<section class="card space-y-2 p-4">
			<h2>{t('wifi.unifi.phonesTitle')}</h2>
			<label class="flex items-start gap-2">
				<input
					type="checkbox"
					class="mt-0.5"
					checked={view.connection.phone_network}
					disabled={busy}
					onchange={(e) => setPhoneNetwork(e.currentTarget.checked)}
				/>
				<span>
					<span class="font-medium">{t('wifi.unifi.phoneNetwork')}</span>
					<span class="block text-xs text-slate-500 dark:text-slate-400"
						>{t('wifi.unifi.phoneNetworkHint')}</span
					>
				</span>
			</label>
		</section>

		<form class="card space-y-3 p-4" onsubmit={saveVoice}>
			<h2>{t('unifiWrite.voiceTitle')}</h2>
			<p class="text-sm text-slate-500 dark:text-slate-400">{t('unifiWrite.voiceIntro')}</p>
			<div>
				<label for="unifi-voice">{t('unifiWrite.voiceSsid')}</label>
				<div class="flex gap-2">
					<select id="unifi-voice" class="input" bind:value={voice.wifi_id}>
						<option value="">{t('unifiWrite.noSsid')}</option>
						{#if view.connection.voice_wifi_id && !ssids?.some((w) => w.id === view?.connection?.voice_wifi_id)}
							<option value={view.connection.voice_wifi_id}
								>{view.connection.voice_wifi_name ?? view.connection.voice_wifi_id}</option
							>
						{/if}
						{#each ssids ?? [] as w (w.id)}
							<option value={w.id}
								>{w.name}{w.type !== 'STANDARD' ? ` (${t('unifiWrite.iot')})` : ''}{w.origin &&
								w.origin !== 'USER_DEFINED'
									? ` (${t('unifiWrite.central')})`
									: ''}</option
							>
						{/each}
					</select>
					<button type="button" class="btn shrink-0" disabled={busy} onclick={loadSsids}
						>{t('unifiWrite.loadSsids')}</button
					>
				</div>
			</div>
			<label class="flex items-start gap-2">
				<input
					type="checkbox"
					class="mt-0.5"
					bind:checked={voice.write_enabled}
					disabled={!canSwitch && !voice.write_enabled}
				/>
				<span>
					<span class="font-medium">{t('unifiWrite.writeSwitch')}</span>
					<span class="block text-xs text-slate-500 dark:text-slate-400"
						>{t('unifiWrite.writeHint')}</span
					>
					<span class="block text-xs text-slate-500 dark:text-slate-400"
						>{t('unifiWrite.writeVersion', {
							min: caps.write_min_version || '10.6.106',
							version: view.connection.version || '—'
						})}</span
					>
				</span>
			</label>
			<div>
				<button
					class="btn btn-primary"
					disabled={busy || (view.connection.write_enabled && !can('unifi.write'))}
					>{t('common.save')}</button
				>
			</div>
		</form>
	{/if}
</div>
