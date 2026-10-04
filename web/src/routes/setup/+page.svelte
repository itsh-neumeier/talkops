<script lang="ts">
	import { goto } from '$app/navigation';
	import { api, type Me } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { setSession } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let username = $state('admin');
	let display_name = $state('Administrator');
	let password = $state('');
	let error = $state('');
	let busy = $state(false);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			setSession(await api.post<Me>('/setup', { username, display_name, password }));
			goto('/');
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<form class="card mx-auto mt-10 max-w-md space-y-4" onsubmit={submit}>
	<h1>{t('setup.title')}</h1>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('setup.intro')}</p>
	<ErrorBox {error} />
	<div>
		<label for="username">{t('users.username')}</label>
		<input id="username" class="input" bind:value={username} autocomplete="username" required />
	</div>
	<div>
		<label for="display_name">{t('setup.displayName')}</label>
		<input id="display_name" class="input" bind:value={display_name} required />
	</div>
	<div>
		<label for="password">{t('users.password')}</label>
		<input
			id="password"
			class="input"
			type="password"
			bind:value={password}
			minlength="10"
			autocomplete="new-password"
			required
		/>
		<p class="hint">{t('setup.passwordHint')}</p>
	</div>
	<button class="btn btn-primary w-full" disabled={busy}>{t('setup.submit')}</button>
</form>
