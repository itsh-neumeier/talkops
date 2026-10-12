<script lang="ts">
	import { goto } from '$app/navigation';
	import { api, type Me } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { setSession } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	import RestoreAssistant from '#lib/components/RestoreAssistant.svelte';
	let mode = $state<'choose' | 'new' | 'restore'>('choose');

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

<div class="mx-auto mt-10 max-w-2xl space-y-4">
	<div class="card space-y-3">
		<h1>{t('setup.title')}</h1>
		<p>{t('setup.choose')}</p>
		<div class="flex flex-wrap gap-2">
			<button class="btn" class:btn-primary={mode === 'new'} onclick={() => (mode = 'new')}
				>{t('setup.new')}</button
			>
			<button class="btn" class:btn-primary={mode === 'restore'} onclick={() => (mode = 'restore')}
				>{t('setup.restore')}</button
			>
		</div>
	</div>
	{#if mode === 'restore'}
		<RestoreAssistant />
	{:else if mode === 'new'}
		<form class="card space-y-4" onsubmit={submit}>
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
	{/if}
</div>
