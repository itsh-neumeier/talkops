<script lang="ts">
	// Footer that stays at the bottom of the window: copyright and version
	// for every user, the system load for users with diagnostics access.
	// The load refreshes while the tab is visible.
	import { onMount } from 'svelte';
	import { api, fetchStatus, type SystemLoad } from '#lib/api.ts';
	import { i18n, t } from '#lib/i18n/index.svelte.ts';
	import { formatBytes, formatUptime } from '#lib/phoneNetwork.ts';
	import { can } from '#lib/session.svelte.ts';

	const REFRESH_MS = 15_000;
	const year = new Date().getFullYear();
	let version = $state('');
	let load = $state<SystemLoad | null>(null);

	const number = (value: number, digits: number) =>
		value.toLocaleString(i18n.locale, {
			minimumFractionDigits: digits,
			maximumFractionDigits: digits
		});
	const memPercent = $derived(
		load && load.mem_total_bytes > 0
			? Math.round((load.mem_used_bytes / load.mem_total_bytes) * 100)
			: null
	);
	// Amber from 80 %, red from 95 % (CPU, memory, load per core).
	function level(ratio: number | null) {
		if (ratio == null || ratio < 0.8) return '';
		return ratio < 0.95 ? 'text-amber-700 dark:text-amber-400' : 'text-red-700 dark:text-red-400';
	}

	async function refresh() {
		if (!can('diagnostics.manage') || document.hidden) return;
		const value = await api.get<SystemLoad>('/system/load').catch(() => null);
		load = typeof value?.cpus === 'number' ? value : null;
	}

	onMount(() => {
		fetchStatus()
			.then((s) => (version = typeof s?.version === 'string' ? s.version : ''))
			.catch(() => {});
		void refresh();
		// The first sample has no CPU value; the second one follows soon.
		const first = setTimeout(refresh, 2_000);
		const timer = setInterval(refresh, REFRESH_MS);
		const onVisible = () => !document.hidden && void refresh();
		document.addEventListener('visibilitychange', onVisible);
		return () => {
			clearTimeout(first);
			clearInterval(timer);
			document.removeEventListener('visibilitychange', onVisible);
		};
	});
</script>

<footer
	class="app-footer fixed inset-x-0 bottom-0 z-20 flex h-(--footer-h) items-center justify-between gap-4 overflow-hidden border-t border-slate-200 bg-white/95 px-4 text-xs whitespace-nowrap text-slate-500 backdrop-blur dark:border-slate-800 dark:bg-slate-900/95 dark:text-slate-400"
	data-testid="app-footer"
>
	<span class="truncate"
		>© {year} ITSH Neumeier · TalkOps{#if version}&nbsp;{version}{/if}</span
	>
	{#if load}
		<dl class="flex shrink-0 items-center gap-3 tabular-nums" aria-label={t('footer.load')}>
			<div class="flex gap-1 {level(load.cpu_percent == null ? null : load.cpu_percent / 100)}">
				<dt>{t('footer.cpu')}</dt>
				<dd>{load.cpu_percent == null ? '—' : `${number(load.cpu_percent, 0)} %`}</dd>
			</div>
			<div
				class="hidden gap-1 sm:flex {level(load.load1 / Math.max(load.cpus, 1))}"
				title={t('footer.loadHint', { cpus: load.cpus })}
			>
				<dt>{t('footer.loadAvg')}</dt>
				<dd>
					{number(load.load1, 2)} / {number(load.load5, 2)} / {number(load.load15, 2)}
				</dd>
			</div>
			<div
				class="flex gap-1 {level(memPercent == null ? null : memPercent / 100)}"
				title="{formatBytes(load.mem_used_bytes)} / {formatBytes(load.mem_total_bytes)}"
			>
				<dt>{t('footer.memory')}</dt>
				<dd>{memPercent == null ? '—' : `${memPercent} %`}</dd>
			</div>
			<div class="hidden gap-1 md:flex">
				<dt>{t('footer.uptime')}</dt>
				<dd>{formatUptime(load.uptime_secs)}</dd>
			</div>
		</dl>
	{/if}
</footer>
