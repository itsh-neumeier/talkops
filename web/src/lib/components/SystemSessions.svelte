<script lang="ts">
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let error = $state('');
	let info = $state('');
	let busy = $state(false);

	async function reset(action: 'hangup' | 'reregister' | 'all') {
		const question = {
			hangup: t('diag.hangupConfirm'),
			reregister: t('diag.reregisterConfirm'),
			all: t('diag.sessionsAllConfirm')
		}[action];
		if (!confirm(question)) return;
		error = '';
		info = '';
		busy = true;
		try {
			const r = await api.post<{ hung_up: number; reregistered: boolean }>(
				'/diagnostics/sessions',
				{ action }
			);
			info = [
				action !== 'reregister' ? t('diag.hungUp', { n: r.hung_up }) : '',
				r.reregistered ? t('diag.reregistered') : ''
			]
				.filter(Boolean)
				.join(' ');
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<section class="card space-y-3">
	<h2>{t('diag.sessions')}</h2>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('diag.sessionsHint')}</p>
	<ErrorBox {error} />
	{#if info}<p class="text-sm text-emerald-700 dark:text-emerald-400" data-testid="sessions-info">
			{info}
		</p>{/if}
	<div class="flex flex-wrap gap-2">
		<button class="btn" disabled={busy} onclick={() => reset('hangup')}
			>{t('diag.hangupAll')}</button
		>
		<button class="btn" disabled={busy} onclick={() => reset('reregister')}
			>{t('diag.reregister')}</button
		>
		<button class="btn btn-danger" disabled={busy} onclick={() => reset('all')}
			>{t('diag.sessionsAll')}</button
		>
	</div>
	<p class="hint">{t('diag.sessionsTimer')}</p>
</section>
