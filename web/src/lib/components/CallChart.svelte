<script lang="ts">
	// Calls per time slot: answered inbound, missed, outbound, internal.
	// Hour and day: stacked bars; week and month: stacked smooth areas with
	// a gradient. Hover, touch or the arrow keys show a tooltip with the
	// values of a slot; the legend shows the totals and hides a series.
	// Plain SVG that scales with its container.
	import type { CallStats } from '#lib/api.ts';
	import { areaPath, niceScale, smoothPath, stack } from '#lib/charts.ts';
	import { i18n, t } from '#lib/i18n/index.svelte.ts';

	let { stats }: { stats: CallStats } = $props();

	const series = [
		{ key: 'answered', color: 'text-teal-600 dark:text-teal-400', label: 'dash.chart.inbound' },
		{ key: 'missed', color: 'text-red-500 dark:text-red-400', label: 'dash.chart.missed' },
		{ key: 'outbound', color: 'text-sky-500 dark:text-sky-400', label: 'dash.chart.outbound' },
		{ key: 'internal', color: 'text-slate-400 dark:text-slate-500', label: 'dash.chart.internal' }
	] as const;
	type Key = (typeof series)[number]['key'];

	let width = $state(640);
	const W = $derived(Math.max(280, Math.round(width)));
	const H = 220;
	const PAD = { left: 30, right: 8, top: 10, bottom: 24 };
	const plotW = $derived(W - PAD.left - PAD.right);
	const plotH = H - PAD.top - PAD.bottom;

	const uid = $props.id();
	let hidden = $state<Key[]>([]);
	let hover = $state<number | null>(null);
	const shown = $derived(series.filter((s) => !hidden.includes(s.key)));
	const area = $derived(stats.range === '1w' || stats.range === '1m');

	const rows = $derived(
		stats.buckets.map((b) => ({
			start: new Date(b.start),
			values: {
				answered: b.inbound - b.missed,
				missed: b.missed,
				outbound: b.outbound,
				internal: b.internal
			} as Record<Key, number>
		}))
	);
	const totals = $derived(
		Object.fromEntries(
			series.map((s) => [s.key, rows.reduce((sum, r) => sum + r.values[s.key], 0)])
		) as Record<Key, number>
	);
	const stacked = $derived(stack(shown.map((s) => rows.map((r) => r.values[s.key]))));
	const scale = $derived(niceScale(stacked.at(-1) ?? []));
	const grid = $derived(
		Array.from({ length: scale.max / scale.step + 1 }, (_, i) => i * scale.step)
	);
	const slot = $derived(plotW / Math.max(1, rows.length));
	const y = (v: number) => PAD.top + plotH - (v / scale.max) * plotH;
	const cx = (i: number) => PAD.left + i * slot + slot / 2;

	/** A bar segment; only the top segment of a column has round corners. */
	function bar(x: number, top: number, w: number, h: number, round: boolean): string {
		const r = round ? Math.min(3, w / 2, h) : 0;
		return `M${x},${top + h}V${top + r}Q${x},${top} ${x + r},${top}H${x + w - r}Q${x + w},${top} ${x + w},${top + r}V${top + h}Z`;
	}

	// Area mode: points of each stacked line from the first to the last slot.
	const lines = $derived(stacked.map((values) => values.map((v, i) => ({ x: cx(i), y: y(v) }))));
	/** The band of series `k`: its line, back along the line below it. */
	function band(k: number): string {
		if (k === 0) return areaPath(lines[0], y(0));
		const below = [...lines[k - 1]]
			.reverse()
			.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`)
			.join('L');
		return `${smoothPath(lines[k])}L${below}Z`;
	}

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
	function slotTitle(i: number) {
		const start = rows[i].start;
		const next = rows[i + 1]?.start ?? new Date(stats.end);
		if (stats.range === '1h' || stats.range === '1d') return `${label(start)}–${label(next)}`;
		return start.toLocaleDateString(locale, {
			weekday: 'short',
			day: '2-digit',
			month: '2-digit'
		});
	}
	// Label every n-th slot so they do not overlap.
	const every = $derived(Math.max(1, Math.ceil(rows.length / Math.max(3, Math.floor(plotW / 70)))));

	function toggle(key: Key) {
		hidden = hidden.includes(key) ? hidden.filter((k) => k !== key) : [...hidden, key];
	}
	function onkeydown(e: KeyboardEvent) {
		if (!rows.length) return;
		if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
			e.preventDefault();
			const d = e.key === 'ArrowRight' ? 1 : -1;
			hover =
				hover == null ? (d > 0 ? 0 : rows.length - 1) : (hover + d + rows.length) % rows.length;
		} else if (e.key === 'Escape') hover = null;
	}
	const tipLeft = $derived(hover == null ? 0 : (cx(hover) / W) * 100);
</script>

<figure class="space-y-3">
	<!-- The arrow keys move the tooltip from slot to slot. -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div
		class="relative rounded-lg focus-visible:outline-2 focus-visible:outline-teal-500"
		bind:clientWidth={width}
		role="group"
		aria-label={t('dash.chart.title')}
		tabindex="0"
		{onkeydown}
		onblur={() => (hover = null)}
		onpointerleave={() => (hover = null)}
	>
		<svg viewBox="0 0 {W} {H}" class="h-auto w-full select-none" aria-hidden="true">
			<defs>
				{#each series as s (s.key)}
					<linearGradient id="call-chart-{uid}-{s.key}" x1="0" x2="0" y1="0" y2="1" class={s.color}>
						<stop offset="0%" style="stop-color: currentColor" stop-opacity="0.45" />
						<stop offset="100%" style="stop-color: currentColor" stop-opacity="0.04" />
					</linearGradient>
				{/each}
			</defs>
			{#each grid as v (v)}
				<line
					x1={PAD.left}
					x2={W - PAD.right}
					y1={y(v)}
					y2={y(v)}
					class={v === 0
						? 'stroke-slate-300 dark:stroke-slate-600'
						: 'stroke-slate-200 dark:stroke-slate-700/70'}
					stroke-dasharray={v === 0 ? undefined : '3 4'}
					stroke-width="1"
				/>
				<text x={PAD.left - 8} y={y(v) + 3} text-anchor="end" class="fill-slate-400 text-[10px]"
					>{v}</text
				>
			{/each}
			{#if hover != null}
				<rect
					x={PAD.left + hover * slot}
					y={PAD.top}
					width={slot}
					height={plotH}
					rx="4"
					class="fill-slate-100 dark:fill-slate-800/70"
				/>
			{/if}
			{#if area}
				{#each shown as s, k (s.key)}
					<path d={band(k)} fill="url(#call-chart-{uid}-{s.key})" />
				{/each}
				{#each shown as s, k (s.key)}
					<path
						d={smoothPath(lines[k])}
						class="fill-none stroke-current {s.color}"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					/>
				{/each}
				{#if hover != null}
					<line
						x1={cx(hover)}
						x2={cx(hover)}
						y1={PAD.top}
						y2={y(0)}
						class="stroke-slate-400 dark:stroke-slate-500"
						stroke-dasharray="2 3"
					/>
					{#each shown as s, k (s.key)}
						<circle
							cx={cx(hover)}
							cy={lines[k][hover].y}
							r="3.5"
							class="fill-current stroke-white dark:stroke-slate-900 {s.color}"
							stroke-width="1.5"
						/>
					{/each}
				{/if}
			{:else}
				{#each rows as r, i (i)}
					{@const w = Math.max(2, Math.min(28, slot * 0.62))}
					{@const x = cx(i) - w / 2}
					{@const top = shown.findLastIndex((s) => r.values[s.key] > 0)}
					{#each shown as s, k (s.key)}
						{#if r.values[s.key] > 0}
							<path
								d={bar(
									x,
									y(stacked[k][i]),
									w,
									y(stacked[k][i] - r.values[s.key]) - y(stacked[k][i]),
									k === top
								)}
								class="fill-current transition-opacity {s.color} {hover != null && hover !== i
									? 'opacity-50'
									: ''}"
							/>
						{/if}
					{/each}
				{/each}
			{/if}
			{#each rows as r, i (i)}
				{#if i % every === 0}
					<text
						x={cx(i)}
						y={H - 6}
						text-anchor="middle"
						class="text-[10px] {hover === i
							? 'fill-slate-700 font-semibold dark:fill-slate-200'
							: 'fill-slate-400'}">{label(r.start)}</text
					>
				{/if}
				<!-- Pointer only: the group above handles the keys; the SVG is hidden from assistive technology. -->
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<rect
					x={PAD.left + i * slot}
					y={PAD.top}
					width={slot}
					height={plotH + PAD.bottom}
					fill="transparent"
					data-slot={i}
					onpointerenter={() => (hover = i)}
					onpointerdown={() => (hover = i)}
				/>
			{/each}
		</svg>
		{#if hover != null && rows[hover]}
			<div
				class="pointer-events-none absolute top-0 z-10 min-w-36 rounded-lg border border-slate-200 bg-white/95 px-3 py-2 text-xs shadow-lg dark:border-slate-700 dark:bg-slate-900/95"
				style="left: {tipLeft}%; transform: translateX({tipLeft > 60
					? 'calc(-100% - 12px)'
					: '12px'})"
				role="status"
				data-testid="chart-tooltip"
			>
				<p class="mb-1 font-semibold text-slate-700 dark:text-slate-200">{slotTitle(hover)}</p>
				{#each shown as s (s.key)}
					<p class="flex items-center justify-between gap-4">
						<span class="inline-flex items-center gap-1.5 text-slate-600 dark:text-slate-300"
							><span class="h-2 w-2 rounded-full bg-current {s.color}"></span>{t(s.label)}</span
						>
						<span class="font-medium tabular-nums">{rows[hover].values[s.key]}</span>
					</p>
				{/each}
			</div>
		{/if}
	</div>
	<figcaption class="flex flex-wrap gap-2 text-xs">
		{#each series as s (s.key)}
			{@const off = hidden.includes(s.key)}
			<button
				type="button"
				class="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 transition-colors {off
					? 'border-slate-200 text-slate-400 line-through dark:border-slate-700 dark:text-slate-500'
					: 'border-slate-200 bg-slate-50 text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700'}"
				aria-pressed={!off}
				onclick={() => toggle(s.key)}
			>
				<span
					class="h-2.5 w-2.5 rounded-full {off
						? 'bg-slate-300 dark:bg-slate-600'
						: `bg-current ${s.color}`}"
				></span>
				{t(s.label)}
				<span class="font-semibold tabular-nums">{totals[s.key]}</span>
			</button>
		{/each}
	</figcaption>
</figure>
