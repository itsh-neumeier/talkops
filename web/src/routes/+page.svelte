<script lang="ts">
	import { onMount } from 'svelte';
	import { fetchStatus, type SystemStatus } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';

	let status = $state<SystemStatus | null>(null);
	let failed = $state(false);

	async function refresh() {
		try {
			status = await fetchStatus();
			failed = false;
		} catch {
			failed = true;
		}
	}

	onMount(() => {
		refresh();
		const timer = setInterval(refresh, 10_000);
		return () => clearInterval(timer);
	});
</script>

<section
	class="rounded-xl border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900"
>
	<h1 class="mb-4 text-lg font-semibold">{t('status.title')}</h1>
	{#if failed}
		<p class="text-red-600 dark:text-red-400">{t('status.error')}</p>
	{:else if !status}
		<p class="text-slate-500">{t('status.loading')}</p>
	{:else}
		<dl class="grid gap-3 sm:grid-cols-3">
			<div>
				<dt class="text-sm text-slate-500 dark:text-slate-400">{t('status.version')}</dt>
				<dd class="font-mono">{status.version}</dd>
			</div>
			{#each [['status.database', status.database], ['status.freeswitch', status.freeswitch]] as const as [label, component] (label)}
				<div>
					<dt class="text-sm text-slate-500 dark:text-slate-400">{t(label)}</dt>
					<dd class="flex items-center gap-2">
						<span
							class="inline-block h-2.5 w-2.5 rounded-full {component.ok
								? 'bg-emerald-500'
								: 'bg-red-500'}"
						></span>
						{component.ok ? t('status.ok') : t('status.down')}
					</dd>
					{#if component.detail}
						<dd class="text-xs text-slate-500 dark:text-slate-400">{component.detail}</dd>
					{/if}
				</div>
			{/each}
		</dl>
	{/if}
</section>
