<script lang="ts">
	import { goto } from '$app/navigation';
	import { ApiError, api, type Me } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { setSession } from '#lib/session.svelte.ts';

	type LoginAnswer = Me | { mfa_required: true; mfa_token: string };

	let username = $state('');
	let password = $state('');
	let code = $state('');
	let mfaToken = $state<string | null>(null);
	let error = $state('');
	let busy = $state(false);

	function failed(err: unknown, key: 'login.failed' | 'login.codeFailed') {
		error = err instanceof ApiError && err.status === 429 ? t('login.rateLimited') : t(key);
	}

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			const answer = await api.post<LoginAnswer>('/auth/login', { username, password });
			if ('mfa_required' in answer) {
				mfaToken = answer.mfa_token;
				password = '';
			} else {
				setSession(answer);
				goto('/');
			}
		} catch (err) {
			failed(err, 'login.failed');
		} finally {
			busy = false;
		}
	}

	async function submitCode(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			setSession(await api.post<Me>('/auth/login/totp', { mfa_token: mfaToken, code }));
			goto('/');
		} catch (err) {
			code = '';
			failed(err, 'login.codeFailed');
			// After too many attempts the server forgets the challenge.
			if (err instanceof ApiError && err.status === 401) error = t('login.codeFailed');
		} finally {
			busy = false;
		}
	}
</script>

{#if mfaToken}
	<form class="card mx-auto mt-10 max-w-sm space-y-4" onsubmit={submitCode}>
		<h1>{t('login.codeTitle')}</h1>
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('login.codeHint')}</p>
		<ErrorBox {error} />
		<div>
			<label for="code">{t('login.code')}</label>
			<!-- svelte-ignore a11y_autofocus -->
			<input
				id="code"
				class="input font-mono tracking-widest"
				bind:value={code}
				autocomplete="one-time-code"
				inputmode="text"
				maxlength="12"
				autofocus
				required
			/>
		</div>
		<button class="btn btn-primary w-full" disabled={busy}>{t('login.verify')}</button>
		<button
			type="button"
			class="btn w-full"
			onclick={() => {
				mfaToken = null;
				code = '';
				error = '';
			}}>{t('common.back')}</button
		>
	</form>
{:else}
	<form class="card mx-auto mt-10 max-w-sm space-y-4" onsubmit={submit}>
		<h1>{t('login.title')}</h1>
		<ErrorBox {error} />
		<div>
			<label for="username">{t('login.username')}</label>
			<input id="username" class="input" bind:value={username} autocomplete="username" required />
		</div>
		<div>
			<label for="password">{t('login.password')}</label>
			<input
				id="password"
				class="input"
				type="password"
				bind:value={password}
				autocomplete="current-password"
				required
			/>
		</div>
		<button class="btn btn-primary w-full" disabled={busy}>{t('login.submit')}</button>
	</form>
{/if}
