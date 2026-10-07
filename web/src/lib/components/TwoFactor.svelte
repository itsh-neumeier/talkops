<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	type Status = { enabled: boolean; recovery_codes_left: number; available: boolean };
	type Setup = { secret: string; otpauth_uri: string; qr_svg: string };

	let status = $state<Status | null>(null);
	let setup = $state<Setup | null>(null);
	let code = $state('');
	let password = $state('');
	let recovery = $state<string[] | null>(null);
	let error = $state('');

	async function load() {
		try {
			status = await api.get<Status>('/auth/totp');
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	async function begin() {
		error = '';
		try {
			setup = await api.post<Setup>('/auth/totp/setup', {});
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function enable(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		try {
			const r = await api.post<{ recovery_codes: string[] }>('/auth/totp/enable', { code });
			recovery = r.recovery_codes;
			setup = null;
			code = '';
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function disable(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		try {
			await api.post('/auth/totp/disable', { password });
			password = '';
			recovery = null;
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	const qr = (svg: string) => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
</script>

<section class="card max-w-md space-y-3">
	<h2>{t('mfa.title')}</h2>
	<ErrorBox {error} />
	{#if status && !status.available}
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('mfa.notAvailable')}</p>
	{:else if status?.enabled}
		<p class="text-sm">
			<span class="badge badge-ok">{t('mfa.active')}</span>
			{t('mfa.codesLeft', { n: status.recovery_codes_left })}
		</p>
		{#if recovery}
			<div
				class="space-y-2 rounded-md border border-amber-300 bg-amber-50 p-3 dark:border-amber-700 dark:bg-amber-950/40"
			>
				<p class="text-sm font-medium">{t('mfa.recoveryTitle')}</p>
				<p class="text-sm">{t('mfa.recoveryHint')}</p>
				<ul class="grid grid-cols-2 gap-1 font-mono text-sm">
					{#each recovery as c (c)}<li>{c}</li>{/each}
				</ul>
			</div>
		{/if}
		<form class="space-y-2" onsubmit={disable}>
			<label for="mfa-pw">{t('mfa.disableHint')}</label>
			<input
				id="mfa-pw"
				class="input"
				type="password"
				bind:value={password}
				autocomplete="current-password"
				required
			/>
			<button class="btn btn-danger">{t('mfa.disable')}</button>
		</form>
	{:else if setup}
		<ol class="list-decimal space-y-2 pl-5 text-sm">
			<li>{t('mfa.step1')}</li>
			<li>{t('mfa.step2')}</li>
		</ol>
		<img
			class="mx-auto h-48 w-48 rounded bg-white p-1"
			src={qr(setup.qr_svg)}
			alt={t('mfa.qrAlt')}
		/>
		<p class="text-center text-xs text-slate-500">
			{t('mfa.manual')}<br /><code class="font-mono break-all">{setup.secret}</code>
		</p>
		<form class="space-y-2" onsubmit={enable}>
			<label for="mfa-code">{t('login.code')}</label>
			<input
				id="mfa-code"
				class="input w-40 font-mono tracking-widest"
				bind:value={code}
				inputmode="numeric"
				autocomplete="one-time-code"
				maxlength="7"
				required
			/>
			<div class="flex gap-2">
				<button class="btn btn-primary">{t('mfa.enable')}</button>
				<button type="button" class="btn" onclick={() => (setup = null)}
					>{t('common.cancel')}</button
				>
			</div>
		</form>
	{:else if status}
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('mfa.hint')}</p>
		<button class="btn" onclick={begin}>{t('mfa.setup')}</button>
	{/if}
</section>
