<script lang="ts">
	// Network state of one phone on its detail page, from the Ubiquiti
	// Networks API. Shows nothing while the feature is off.
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import { formatDateTime, t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import {
		REFRESH_MS,
		formatBytes,
		formatUptime,
		linkSpeed,
		type PhoneNetworkView
	} from '#lib/phoneNetwork.ts';
	import { band, mbit } from '#lib/wifiSurvey.ts';
	import PhoneNetworkCell from './PhoneNetworkCell.svelte';

	let { phoneId }: { phoneId: string } = $props();
	let view = $state<PhoneNetworkView | null>(null);

	async function load() {
		view = await api.get<PhoneNetworkView>('/phones/network').catch(() => view);
	}
	onMount(() => {
		void load();
		const timer = setInterval(() => {
			if (view?.state === 'ok' && !document.hidden) void load();
		}, REFRESH_MS);
		return () => clearInterval(timer);
	});

	const data = $derived(view?.phones.find((p) => p.phone_id === phoneId));
	const c = $derived(data?.client);
	const rows = $derived.by((): [MessageKey, string][] => {
		if (!c) return [];
		const common: [MessageKey, string][] = [
			['phones.network.ip', c.ip ?? '—'],
			[
				'phones.network.lan',
				[c.network, c.vlan != null ? `VLAN ${c.vlan}` : null].filter(Boolean).join(' · ') || '—'
			],
			['phones.network.uptime', formatUptime(c.uptime_secs)],
			['phones.network.traffic', `↑ ${formatBytes(c.tx_bytes)} · ↓ ${formatBytes(c.rx_bytes)}`],
			['phones.network.experience', c.satisfaction != null ? `${c.satisfaction} %` : '—']
		];
		if (c.wired) {
			return [
				['phones.network.switch', data?.switch_name ?? c.sw_mac ?? '—'],
				['phones.network.portLabel', c.sw_port != null ? String(c.sw_port) : '—'],
				['phones.network.speed', linkSpeed(c.wired_rate_mbps)],
				...common
			];
		}
		return [
			['phones.network.ap', data?.ap_name ?? c.ap_mac ?? '—'],
			['phones.network.ssid', c.essid ?? '—'],
			[
				'phones.network.band',
				`${band(c.radio)}${c.channel != null ? ` · ${t('phones.network.channel', { channel: c.channel })}` : ''}`
			],
			['phones.network.rssi', c.rssi != null ? `${c.rssi} dBm` : '—'],
			['phones.network.rates', `↑ ${mbit(c.tx_rate_kbps)} · ↓ ${mbit(c.rx_rate_kbps)} Mbit/s`],
			...common
		];
	});
</script>

{#if view && (view.state === 'ok' || view.state === 'error')}
	<section class="card space-y-3" data-testid="phone-network">
		<div class="flex flex-wrap items-baseline justify-between gap-2">
			<h2>{t('phones.network.title')}</h2>
			{#if view.read_at}
				<span class="text-xs text-slate-500 dark:text-slate-400"
					>{t('phones.network.readAt', { time: formatDateTime(view.read_at) })}</span
				>
			{/if}
		</div>
		{#if view.state === 'error'}
			<p class="text-sm text-amber-700 dark:text-amber-400">
				{t('phones.network.error')}
			</p>
		{:else if !c}
			<p class="text-sm text-slate-500 dark:text-slate-400">{t('phones.network.notConnected')}</p>
		{:else}
			<PhoneNetworkCell {data} />
			<dl class="grid grid-cols-1 gap-x-6 gap-y-1 text-sm @lg:grid-cols-2">
				{#each rows as [key, value] (key)}
					<div
						class="flex justify-between gap-3 border-b border-slate-100 py-1 dark:border-slate-800"
					>
						<dt class="text-slate-500 dark:text-slate-400">{t(key)}</dt>
						<dd class="text-right tabular-nums">{value}</dd>
					</div>
				{/each}
			</dl>
		{/if}
	</section>
{/if}
