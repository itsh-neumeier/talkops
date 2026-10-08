<script lang="ts">
	// Calls per time slot as stacked bars: answered inbound, missed,
	// outbound, internal. Plain SVG, scales with its container.
	import type { CallStats } from '#lib/api.ts';
	import { i18n, t } from '#lib/i18n/index.svelte.ts';

	let { stats }: { stats: CallStats } = $props();

	const W = 640;
	const H = 200;
	const PAD = { left: 28, right: 4, top: 8, bottom: 22 };
	const series = [
		{ key: 'answered', color: 'fill-teal-600 dark:fill-teal-500', label: 'dash.chart.inbound' },
		{ key: 'missed', color: 'fill-red-500', label: 'dash.chart.missed' },
		{ key: 'outbound', color: 'fill-sky-500', label: 'dash.chart.outbound' },
		{ key: 'internal', color: 'fill-slate-400', label: 'dash.chart.internal' }
	] as const;

	const rows = $derived(
		stats.buckets.map((b) => ({
			start: new Date(b.start),
			values: {
				answered: b.inbound - b.missed,
				missed: b.missed,
				outbound: b.outbound,
				internal: b.internal
			},
			total: b.inbound + b.outbound + b.internal
		}))
	);
	// A round axis maximum (1, 2, 5, 10, 20 …), at least 4.
	const max = $derived.by(() => {
		const top = Math.max(4, ...rows.map((r) => r.total));
		const step = 10 ** Math.floor(Math.log10(top));
		return [1, 2, 5, 10].map((m) => m * step).find((v) => v >= top) ?? top;
	});
	const plotW = W - PAD.left - PAD.right;
	const plotH = H - PAD.top - PAD.bottom;
	const slot = $derived(plotW / Math.max(1, rows.length));
	const y = (v: number) => PAD.top + plotH - (v / max) * plotH;

	const locale = $derived(i18n.locale === 'de' ? 'de-DE' : 'en-GB');
	function label(d: Date) {
		switch (stats.range) {
			case '1h':
			case '1d':
				return d.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' });
			case '1w':
				return d.toLocaleDateString(locale, { weekday: 'short' });
			default:
				return d.toLocaleDateString(locale, { day: '2-digit', month: '2-digit' });
		}
	}
	// Label every n-th slot so they do not overlap.
	const every = $derived(Math.max(1, Math.ceil(rows.length / 8)));
</script>

<figure class="space-y-2">
	<svg viewBox="0 0 {W} {H}" class="h-auto w-full" role="img" aria-label={t('dash.chart.title')}>
		{#each [0, 0.25, 0.5, 0.75, 1] as f (f)}
			<line
				x1={PAD.left}
				x2={W - PAD.right}
				y1={y(max * f)}
				y2={y(max * f)}
				class="stroke-slate-200 dark:stroke-slate-700"
				stroke-width="1"
			/>
			<text x={PAD.left - 6} y={y(max * f) + 3} text-anchor="end" class="fill-slate-400 text-[10px]"
				>{Math.round(max * f)}</text
			>
		{/each}
		{#each rows as r, i (i)}
			{@const x = PAD.left + i * slot + slot * 0.15}
			{@const w = slot * 0.7}
			<g>
				<title
					>{label(r.start)}: {r.total} – {series
						.map((s) => `${t(s.label)} ${r.values[s.key]}`)
						.join(', ')}</title
				>
				<rect x={PAD.left + i * slot} y={PAD.top} width={slot} height={plotH} fill="transparent" />
				{#each series as s, k (s.key)}
					{@const below = series.slice(0, k).reduce((sum, p) => sum + r.values[p.key], 0)}
					{#if r.values[s.key] > 0}
						<rect
							{x}
							width={w}
							y={y(below + r.values[s.key])}
							height={y(below) - y(below + r.values[s.key])}
							class={s.color}
							rx="1.5"
						/>
					{/if}
				{/each}
			</g>
			{#if i % every === 0}
				<text
					x={PAD.left + i * slot + slot / 2}
					y={H - 6}
					text-anchor="middle"
					class="fill-slate-400 text-[10px]">{label(r.start)}</text
				>
			{/if}
		{/each}
	</svg>
	<figcaption class="flex flex-wrap gap-x-4 gap-y-1 text-xs text-slate-600 dark:text-slate-300">
		{#each series as s (s.key)}
			<span class="inline-flex items-center gap-1.5">
				<svg viewBox="0 0 10 10" class="h-2.5 w-2.5"
					><rect width="10" height="10" rx="2" class={s.color} /></svg
				>
				{t(s.label)}
			</span>
		{/each}
	</figcaption>
</figure>
