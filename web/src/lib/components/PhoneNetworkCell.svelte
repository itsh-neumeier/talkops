<script lang="ts">
	// Compact network state of one phone for the phone list: signal, access
	// point and band for Wi-Fi; switch, port and speed for a cable.
	import { t } from '#lib/i18n/index.svelte.ts';
	import { linkSpeed, signalBars, signalClass, type PhoneNetwork } from '#lib/phoneNetwork.ts';
	import { band } from '#lib/wifiSurvey.ts';

	let { data }: { data: PhoneNetwork | undefined } = $props();
	const c = $derived(data?.client);
</script>

{#if !data || !c}
	<span class="text-xs text-slate-400">{t('phones.network.offline')}</span>
{:else if c.wired}
	<span class="inline-flex max-w-64 items-start gap-1.5 text-sm">
		<svg
			viewBox="0 0 24 24"
			class="mt-0.5 h-4 w-4 shrink-0 text-sky-600 dark:text-sky-400"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			stroke-linejoin="round"
			aria-label={t('phones.network.wired')}
			><rect x="5" y="9" width="14" height="10" rx="1.5" /><path
				d="M9 9V5h6v4M9 13v2M12 13v2M15 13v2"
			/></svg
		>
		<span class="min-w-0">
			<span class="block truncate" title={data.switch_name ?? c.sw_mac ?? ''}
				>{data.switch_name ?? c.sw_mac ?? '—'}</span
			>
			<span class="block text-xs text-slate-500 dark:text-slate-400"
				>{c.sw_port != null ? t('phones.network.port', { port: c.sw_port }) : ''}{c.sw_port !=
					null && c.wired_rate_mbps != null
					? ' · '
					: ''}{c.wired_rate_mbps != null ? linkSpeed(c.wired_rate_mbps) : ''}</span
			>
		</span>
	</span>
{:else}
	{@const bars = signalBars(c.rssi)}
	<span class="inline-flex max-w-64 items-start gap-1.5 text-sm">
		<svg
			viewBox="0 0 16 16"
			class="mt-0.5 h-4 w-4 shrink-0 {signalClass(c.rssi)}"
			role="img"
			aria-label={t('phones.network.signal', { rssi: c.rssi ?? '—' })}
		>
			{#each [1, 2, 3, 4] as i (i)}
				<rect
					x={i * 3.5 - 2.5}
					y={14 - i * 3}
					width="2.5"
					height={i * 3}
					rx="0.6"
					fill="currentColor"
					opacity={i <= bars ? 1 : 0.2}
				/>
			{/each}
		</svg>
		<span class="min-w-0">
			<span class="block truncate" title={data.ap_name ?? c.ap_mac ?? ''}
				>{c.rssi != null ? `${c.rssi} dBm` : '—'} · {data.ap_name ?? c.ap_mac ?? '—'}</span
			>
			<span class="block text-xs text-slate-500 dark:text-slate-400"
				>{band(c.radio)}{c.channel != null
					? ` · ${t('phones.network.channel', { channel: c.channel })}`
					: ''}{c.essid ? ` · ${c.essid}` : ''}</span
			>
		</span>
	</span>
{/if}
