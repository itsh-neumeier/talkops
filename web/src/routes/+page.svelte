<script lang="ts">
	import { onMount } from 'svelte';
	import {
		api,
		fetchStatus,
		type LiveStatus,
		type SystemStatus,
		type TrunkDetail
	} from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';

	let status = $state<SystemStatus | null>(null);
	let live = $state<LiveStatus | null>(null);
	let trunks = $state<TrunkDetail[]>([]);
	let failed = $state(false);

	async function refresh() {
		try {
			status = await fetchStatus();
			if (hasRole('operator')) {
				[live, trunks] = await Promise.all([
					api.get<LiveStatus>('/telephony/status'),
					api.get<TrunkDetail[]>('/trunks')
				]);
			}
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

	function registered(trunk: TrunkDetail) {
		return trunk.accounts.filter((a) => a.state?.state === 'REGED').length;
	}
</script>

<div class="space-y-4">
	<h1>{t('nav.dashboard')}</h1>
	<section class="card">
		<h2 class="mb-4">{t('status.title')}</h2>
		{#if failed}
			<p class="text-red-600 dark:text-red-400">{t('status.error')}</p>
		{:else if !status}
			<p class="text-slate-500">{t('common.loading')}</p>
		{:else}
			<dl class="grid gap-3 sm:grid-cols-4">
				<div>
					<dt class="text-sm text-slate-500 dark:text-slate-400">{t('status.version')}</dt>
					<dd class="font-mono">{status.version}</dd>
				</div>
				{#each [['status.database', status.database], ['status.freeswitch', status.freeswitch]] as const as [label, component] (label)}
					<div>
						<dt class="text-sm text-slate-500 dark:text-slate-400">{t(label)}</dt>
						<dd>
							<span class="badge {component.ok ? 'badge-ok' : 'badge-bad'}"
								>{component.ok ? t('status.ok') : t('status.down')}</span
							>
						</dd>
					</div>
				{/each}
				{#if live}
					<div>
						<dt class="text-sm text-slate-500 dark:text-slate-400">{t('dash.registrations')}</dt>
						<dd class="text-lg font-semibold">{live.registrations.length}</dd>
					</div>
				{/if}
			</dl>
		{/if}
	</section>

	{#if hasRole('operator')}
		<section class="card">
			<h2 class="mb-3">{t('dash.trunks')}</h2>
			{#if trunks.length === 0}
				<p class="text-sm text-slate-500">{t('dash.noTrunks')}</p>
				{#if hasRole('admin')}<a class="btn btn-primary mt-3" href="/trunks">{t('dash.addTrunk')}</a
					>{/if}
			{:else}
				<ul class="divide-y divide-slate-100 dark:divide-slate-800">
					{#each trunks as trunk (trunk.id)}
						<li class="flex items-center justify-between py-2">
							<a class="hover:underline" href="/trunks/{trunk.id}">{trunk.name}</a>
							{#if !trunk.enabled}
								<span class="badge badge-muted">{t('common.disabled')}</span>
							{:else}
								<span
									class="badge {registered(trunk) === trunk.accounts.length &&
									trunk.accounts.length > 0
										? 'badge-ok'
										: 'badge-bad'}"
									>{registered(trunk)}/{trunk.accounts.length} {t('trunks.registered')}</span
								>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}
		</section>
	{/if}
</div>
