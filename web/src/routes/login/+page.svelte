<script lang="ts">
	import { goto } from '$app/navigation';
	import { ApiError, api, type Me } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { setSession } from '#lib/session.svelte.ts';

	let username = $state('');
	let password = $state('');
	let error = $state('');
	let busy = $state(false);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			setSession(await api.post<Me>('/auth/login', { username, password }));
			goto('/');
		} catch (err) {
			error =
				err instanceof ApiError && err.status === 429 ? t('login.rateLimited') : t('login.failed');
		} finally {
			busy = false;
		}
	}
</script>

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
