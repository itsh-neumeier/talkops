<script lang="ts">
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import type { DiagnosticsView } from '#lib/diagnostics.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let error = $state('');
	let restarting = $state('');

	async function restart(target: 'freeswitch' | 'all') {
		const question = target === 'all' ? t('diag.restartAllConfirm') : t('diag.restartFsConfirm');
		if (!confirm(question)) return;
		error = '';
		try {
			await api.post('/diagnostics/restart', { target });
			restarting = target;
			// Wait until the server (and FreeSWITCH) answer again.
			const started = Date.now();
			const wait = async () => {
				await new Promise((r) => setTimeout(r, 3000));
				try {
					const res = await fetch('/api/v1/status', { cache: 'no-store' });
					const fsUp =
						target === 'freeswitch'
							? (await api.get<DiagnosticsView>('/diagnostics')).freeswitch
							: 'x';
					if (res.ok && fsUp) {
						restarting = '';
						return;
					}
				} catch {
					// not back yet
				}
				if (Date.now() - started < 180_000) await wait();
				else restarting = '';
			};
			await wait();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<section class="card space-y-3">
	<h2>{t('diag.restart')}</h2>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('diag.restartHint')}</p>
	<ErrorBox {error} />
	{#if restarting}
		<p class="text-sm text-amber-700 dark:text-amber-400" data-testid="restarting">
			⟳ {t('diag.restarting')}
		</p>
	{/if}
	<div class="flex flex-wrap gap-2">
		<button class="btn" disabled={!!restarting} onclick={() => restart('freeswitch')}
			>{t('diag.restartFs')}</button
		>
		<button class="btn btn-danger" disabled={!!restarting} onclick={() => restart('all')}
			>{t('diag.restartAll')}</button
		>
	</div>
</section>
