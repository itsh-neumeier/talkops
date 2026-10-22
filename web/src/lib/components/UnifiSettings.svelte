<script lang="ts">
	// Read-only UniFi Network connection (ADR 0041). The API key is stored
	// encrypted and never shown again; an empty field keeps it.
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import type { UnifiConnection } from '#lib/wifiSurvey.ts';

	type View = { connection: UnifiConnection | null; has_api_key: boolean };

	let view = $state<View | null>(null);
	let form = $state({ url: '', site: 'default', cert_sha256: '', api_key: '' });
	let error = $state('');
	let notice = $state('');
	let busy = $state(false);

	function apply(v: View) {
		view = v;
		form = {
			url: v.connection?.url ?? '',
			site: v.connection?.site ?? 'default',
			cert_sha256: v.connection?.cert_sha256 ?? '',
			api_key: ''
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
			error = errorMessage(err);
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

	const caps = $derived(view?.connection?.capabilities ?? {});
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
			</dl>
			{#if view.connection.last_check_at && !caps.classic && !view.connection.last_error}
				<p class="text-sm text-amber-700 dark:text-amber-400">{t('wifi.unifi.noClassic')}</p>
			{/if}
			{#if view.connection.last_error}
				<p class="error-box">{view.connection.last_error}</p>
			{/if}
		</section>
	{/if}
</div>
