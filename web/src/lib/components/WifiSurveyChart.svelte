<script lang="ts">
	// Timeline of a Wi-Fi survey: AP sections as background bands, RSSI as a
	// line, uplink loss for each second as bars, downlink loss from RTCP as
	// dots, roams as vertical lines (color = rating) and markers on top.
	// Plain SVG that scales with its container. A click asks to play the
	// recording from that point.
	import { t } from '#lib/i18n/index.svelte.ts';
	import { apLabel, clock, type Roam, type Timeline } from '#lib/wifiSurvey.ts';

	let {
		timeline,
		roams = [],
		from = 0,
		to,
		onseek
	}: {
		timeline: Timeline;
		roams?: Roam[];
		/** First and last offset (ms) to show; the live view shows the last minutes. */
		from?: number;
		to?: number;
		onseek?: (offsetMs: number) => void;
	} = $props();

	const W = 1000;
	const H = 240;
	const PAD = { left: 40, right: 8, top: 22, bottom: 22 };
	const plotW = W - PAD.left - PAD.right;
	// Upper area: RSSI; lower area: loss bars.
	const RSSI_TOP = PAD.top;
	const RSSI_H = 120;
	const LOSS_TOP = RSSI_TOP + RSSI_H + 12;
	const LOSS_H = H - PAD.bottom - LOSS_TOP;
	const RSSI_MIN = -95;
	const RSSI_MAX = -30;

	const end = $derived(
		Math.max(
			from + 10_000,
			to ??
				Math.max(
					timeline.media.at(-1)?.offset_ms ?? 0,
					timeline.radio.at(-1)?.offset_ms ?? 0,
					roams.at(-1)?.offset_ms ?? 0
				)
		)
	);
	const x = (ms: number) => PAD.left + ((ms - from) / (end - from)) * plotW;
	const inView = (ms: number) => ms >= from && ms <= end;
	const yRssi = (r: number) =>
		RSSI_TOP +
		RSSI_H -
		((Math.max(RSSI_MIN, Math.min(RSSI_MAX, r)) - RSSI_MIN) / (RSSI_MAX - RSSI_MIN)) * RSSI_H;

	const PALETTE = [
		'fill-sky-200/60 dark:fill-sky-900/50',
		'fill-violet-200/60 dark:fill-violet-900/50',
		'fill-amber-200/60 dark:fill-amber-900/50',
		'fill-emerald-200/60 dark:fill-emerald-900/50',
		'fill-rose-200/60 dark:fill-rose-900/50',
		'fill-cyan-200/60 dark:fill-cyan-900/50'
	];
	const apColors = $derived.by(() => {
		const map = new Map<string, string>();
		for (const r of timeline.radio) {
			if (r.ap_mac && !map.has(r.ap_mac)) map.set(r.ap_mac, PALETTE[map.size % PALETTE.length]);
		}
		return map;
	});

	// AP sections: runs of samples on the same AP.
	const sections = $derived.by(() => {
		const out: { start: number; end: number; mac: string; name: string }[] = [];
		for (const r of timeline.radio) {
			if (!r.ap_mac || !inView(r.offset_ms)) continue;
			const last = out.at(-1);
			if (last && last.mac === r.ap_mac) last.end = r.offset_ms;
			else {
				if (last) last.end = r.offset_ms;
				out.push({
					start: r.offset_ms,
					end: r.offset_ms,
					mac: r.ap_mac,
					name: apLabel(r.ap_name, r.ap_mac)
				});
			}
		}
		return out;
	});

	const rssiPath = $derived.by(() => {
		let d = '';
		let pen = false;
		for (const r of timeline.radio) {
			if (!inView(r.offset_ms)) continue;
			if (r.rssi == null) {
				pen = false;
				continue;
			}
			d += `${pen ? 'L' : 'M'}${x(r.offset_ms).toFixed(1)},${yRssi(r.rssi).toFixed(1)}`;
			pen = true;
		}
		return d;
	});

	const bars = $derived(
		timeline.media
			.filter((m) => inView(m.offset_ms) && m.expected > 0)
			.map((m) => {
				const lost = Math.max(0, m.expected - m.received);
				const pct = Math.min(100, (lost * 100) / m.expected);
				const silent = m.received === 0 && m.flaws === 0;
				return { m, pct, silent };
			})
	);
	const barW = $derived(
		Math.max(1, (plotW * (timeline.media[0]?.interval_ms ?? 1000)) / (end - from) - 0.5)
	);

	const ticks = $derived.by(() => {
		const span = end - from;
		const steps = [10, 30, 60, 120, 300, 600, 900, 1800].map((s) => s * 1000);
		const step = steps.find((s) => span / s <= 8) ?? 3_600_000;
		const first = Math.ceil(from / step) * step;
		const out: number[] = [];
		for (let v = first; v <= end; v += step) out.push(v);
		return out;
	});

	const markers = $derived(
		timeline.events.filter((e) => e.kind === 'marker' && inView(e.offset_ms))
	);
	const periods = $derived(
		timeline.events.filter((e) => e.kind === 'error_period' && inView(e.offset_ms))
	);

	function click(e: MouseEvent) {
		if (!onseek) return;
		const svg = e.currentTarget as SVGSVGElement;
		const rect = svg.getBoundingClientRect();
		const px = ((e.clientX - rect.left) / rect.width) * W;
		const ms = from + ((px - PAD.left) / plotW) * (end - from);
		if (ms >= from && ms <= end) onseek(Math.max(0, Math.round(ms)));
	}

	const ratingStroke = (r: string) =>
		r === 'bad'
			? 'stroke-red-600 dark:stroke-red-400'
			: r === 'audible'
				? 'stroke-amber-500'
				: 'stroke-teal-600 dark:stroke-teal-400';
	const markerIcon = (m: string | null) =>
		m === 'disturbance' ? '!' : m === 'location' ? '⌖' : '✎';
</script>

<figure class="space-y-2">
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<svg
		viewBox="0 0 {W} {H}"
		class="h-auto w-full select-none {onseek ? 'cursor-pointer' : ''}"
		role="img"
		aria-label={t('wifi.chartLabel')}
		onclick={click}
	>
		{#each sections as s (s.start)}
			<rect
				x={x(s.start)}
				y={RSSI_TOP}
				width={Math.max(1, x(s.end) - x(s.start))}
				height={RSSI_H}
				class={apColors.get(s.mac)}><title>{s.name}</title></rect
			>
			{#if x(s.end) - x(s.start) > 60}
				<text
					x={x(s.start) + 4}
					y={RSSI_TOP + 11}
					class="fill-slate-600 text-[11px] dark:fill-slate-300">{s.name}</text
				>
			{/if}
		{/each}
		{#each [-40, -55, -67, -75, -85] as level (level)}
			<line
				x1={PAD.left}
				x2={W - PAD.right}
				y1={yRssi(level)}
				y2={yRssi(level)}
				class="stroke-slate-200 dark:stroke-slate-700"
				stroke-dasharray={level === -75 ? '4 3' : undefined}
				stroke-width="1"
			/>
			<text
				x={PAD.left - 4}
				y={yRssi(level) + 3}
				text-anchor="end"
				class="fill-slate-400 text-[10px]">{level}</text
			>
		{/each}
		<path d={rssiPath} class="fill-none stroke-sky-700 dark:stroke-sky-300" stroke-width="2" />

		<line
			x1={PAD.left}
			x2={W - PAD.right}
			y1={LOSS_TOP + LOSS_H}
			y2={LOSS_TOP + LOSS_H}
			class="stroke-slate-300 dark:stroke-slate-600"
		/>
		<text x={PAD.left - 4} y={LOSS_TOP + 8} text-anchor="end" class="fill-slate-400 text-[10px]"
			>%</text
		>
		{#each periods as p (p.id)}
			{@const stop = Number(p.data.end_ms ?? p.offset_ms)}
			<rect
				x={x(p.offset_ms)}
				y={LOSS_TOP}
				width={Math.max(1, x(stop) - x(p.offset_ms))}
				height={LOSS_H}
				class="fill-red-100 dark:fill-red-950/60"><title>{t('wifi.errorPeriod')}</title></rect
			>
		{/each}
		{#each bars as b (b.m.offset_ms)}
			{#if b.silent}
				<rect
					x={x(b.m.offset_ms) - barW}
					y={LOSS_TOP + LOSS_H - 3}
					width={barW}
					height="3"
					class="fill-slate-400"
				/>
			{:else if b.pct > 0}
				{@const h = Math.max(2, (Math.min(20, b.pct) / 20) * LOSS_H)}
				<rect
					x={x(b.m.offset_ms) - barW}
					y={LOSS_TOP + LOSS_H - h}
					width={barW}
					height={h}
					class={b.pct > 5 ? 'fill-red-500' : 'fill-amber-500'}
					><title>{clock(b.m.offset_ms)}: {b.pct.toFixed(1)} %</title></rect
				>
			{/if}
			{#if b.m.rtcp_fraction_lost != null && b.m.rtcp_fraction_lost > 0}
				<circle
					cx={x(b.m.offset_ms)}
					cy={LOSS_TOP + LOSS_H - (Math.min(20, b.m.rtcp_fraction_lost) / 20) * LOSS_H}
					r="2.5"
					class="fill-violet-600 dark:fill-violet-400"
				/>
			{/if}
		{/each}

		{#each roams.filter((r) => inView(r.offset_ms)) as r (r.offset_ms)}
			<line
				x1={x(r.offset_ms)}
				x2={x(r.offset_ms)}
				y1={PAD.top - 6}
				y2={LOSS_TOP + LOSS_H}
				class={ratingStroke(r.rating)}
				stroke-width="2"
				stroke-dasharray={r.flags.includes('band_change') ? '3 3' : undefined}
				><title
					>{clock(r.offset_ms)} · {apLabel(r.from_ap_name, r.from_ap_mac)} → {apLabel(
						r.to_ap_name,
						r.to_ap_mac
					)} · {r.gap_ms} ms</title
				></line
			>
		{/each}
		{#each markers as m (m.id)}
			<g transform="translate({x(m.offset_ms)},{PAD.top - 12})">
				<circle r="8" class="fill-slate-800 dark:fill-slate-100" />
				<text
					text-anchor="middle"
					y="4"
					class="fill-white text-[11px] font-bold dark:fill-slate-900">{markerIcon(m.marker)}</text
				>
				<title
					>{clock(m.offset_ms)} · {t(`wifi.marker.${m.marker ?? 'note'}`)}{m.note
						? `: ${m.note}`
						: ''}</title
				>
			</g>
		{/each}

		{#each ticks as tick (tick)}
			<text x={x(tick)} y={H - 6} text-anchor="middle" class="fill-slate-400 text-[10px]"
				>{clock(tick)}</text
			>
		{/each}
	</svg>
	<figcaption class="flex flex-wrap gap-x-4 gap-y-1 text-xs text-slate-500 dark:text-slate-400">
		<span
			><span class="mr-1 inline-block h-0.5 w-4 bg-sky-700 align-middle dark:bg-sky-300"></span>{t(
				'wifi.legend.rssi'
			)}</span
		>
		<span
			><span class="mr-1 inline-block h-2 w-2 bg-amber-500 align-middle"></span>{t(
				'wifi.legend.uplinkLoss'
			)}</span
		>
		<span
			><span class="mr-1 inline-block h-2 w-2 rounded-full bg-violet-600 align-middle"></span>{t(
				'wifi.legend.downlinkLoss'
			)}</span
		>
		<span
			><span class="mr-1 inline-block h-3 w-0.5 bg-teal-600 align-middle"></span>{t(
				'wifi.legend.roam'
			)}</span
		>
		<span
			><span class="mr-1 inline-block h-2 w-2 bg-slate-400 align-middle"></span>{t(
				'wifi.legend.silence'
			)}</span
		>
		{#if onseek}<span>{t('wifi.legend.clickToPlay')}</span>{/if}
	</figcaption>
</figure>
