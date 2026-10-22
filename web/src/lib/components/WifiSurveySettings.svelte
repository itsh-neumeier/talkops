<script lang="ts">
	// Thresholds of the roam rating, pattern rules, retention and limits.
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import type { Thresholds } from '#lib/wifiSurvey.ts';

	let draft = $state<Thresholds | null>(null);
	let error = $state('');
	let notice = $state('');
	let busy = $state(false);

	const groups: { title: MessageKey; fields: { key: keyof Thresholds; unit: string }[] }[] = [
		{
			title: 'wifi.settings.rating',
			fields: [
				{ key: 'clean_ms', unit: 'ms' },
				{ key: 'bad_ms', unit: 'ms' },
				{ key: 'loss_block_pct', unit: '%' }
			]
		},
		{
			title: 'wifi.settings.patterns',
			fields: [
				{ key: 'sticky_rssi', unit: 'dBm' },
				{ key: 'sticky_secs', unit: 's' },
				{ key: 'pingpong_count', unit: '×' },
				{ key: 'pingpong_secs', unit: 's' },
				{ key: 'weak_rssi', unit: 'dBm' },
				{ key: 'weak_secs', unit: 's' }
			]
		},
		{
			title: 'wifi.settings.limits',
			fields: [
				{ key: 'retention_days', unit: 'd' },
				{ key: 'max_minutes', unit: 'min' },
				{ key: 'max_parallel', unit: '×' }
			]
		}
	];

	onMount(async () => {
		try {
			draft = await api.get<Thresholds>('/wifi-surveys/settings');
		} catch (err) {
			error = errorMessage(err);
		}
	});

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!draft) return;
		busy = true;
		notice = '';
		try {
			draft = await api.put<Thresholds>('/wifi-surveys/settings', draft);
			error = '';
			notice = t('common.saved');
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<ErrorBox {error} />
{#if draft}
	<form class="space-y-4" onsubmit={save}>
		{#each groups as g (g.title)}
			<section class="card space-y-3 p-4">
				<h2>{t(g.title)}</h2>
				<div class="grid grid-cols-1 gap-3 @lg:grid-cols-2 @2xl:grid-cols-3">
					{#each g.fields as f (f.key)}
						<div>
							<label for="ws-{f.key}">{t(`wifi.settings.${f.key}` as MessageKey)} ({f.unit})</label>
							<input
								id="ws-{f.key}"
								class="input"
								type="number"
								required
								bind:value={draft[f.key]}
							/>
						</div>
					{/each}
				</div>
				{#if g.title === 'wifi.settings.limits'}
					<p class="text-sm text-slate-500 dark:text-slate-400">
						{t('wifi.settings.retentionHint')}
					</p>
				{/if}
			</section>
		{/each}
		<div class="flex items-center gap-3">
			<button class="btn btn-primary" disabled={busy}>{t('common.save')}</button>
			{#if notice}<span class="text-sm text-teal-700 dark:text-teal-400" role="status"
					>{notice}</span
				>{/if}
		</div>
		<p class="text-sm text-slate-500 dark:text-slate-400">{t('wifi.settings.reanalyseHint')}</p>
	</form>
{/if}
