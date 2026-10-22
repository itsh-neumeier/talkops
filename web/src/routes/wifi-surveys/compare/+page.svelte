<script lang="ts">
	// Two surveys side by side (for example before and after an AP change).
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { api } from '#lib/api.ts';
	import EmptyRow from '#lib/components/EmptyRow.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import PageHeader from '#lib/components/PageHeader.svelte';
	import { formatDateTime, t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import { clock, ratingBadge, type PairComparison, type Survey } from '#lib/wifiSurvey.ts';

	type Comparison = { a: Survey; b: Survey; pairs: PairComparison[] };
	let data = $state<Comparison | null>(null);
	let error = $state('');

	onMount(async () => {
		const q = page.url.searchParams;
		try {
			data = await api.get<Comparison>(
				`/wifi-surveys/compare?a=${encodeURIComponent(q.get('a') ?? '')}&b=${encodeURIComponent(q.get('b') ?? '')}`
			);
		} catch (err) {
			error = errorMessage(err);
		}
	});

	type Row = {
		key: MessageKey;
		get: (s: Survey) => number | null;
		unit: string;
		better: 'lower' | 'higher';
	};
	const rows: Row[] = [
		{ key: 'wifi.col.roams', get: (s) => s.summary?.roams ?? null, unit: '', better: 'lower' },
		{ key: 'wifi.rating.clean', get: (s) => s.summary?.clean ?? null, unit: '', better: 'higher' },
		{
			key: 'wifi.rating.audible',
			get: (s) => s.summary?.audible ?? null,
			unit: '',
			better: 'lower'
		},
		{ key: 'wifi.rating.bad', get: (s) => s.summary?.bad ?? null, unit: '', better: 'lower' },
		{
			key: 'wifi.worstGap',
			get: (s) => s.summary?.worst_gap_ms ?? null,
			unit: 'ms',
			better: 'lower'
		},
		{ key: 'wifi.loss', get: (s) => s.summary?.loss_pct ?? null, unit: '%', better: 'lower' },
		{ key: 'wifi.tile.mos', get: (s) => s.summary?.mos ?? null, unit: '', better: 'higher' },
		{ key: 'wifi.minRssi', get: (s) => s.summary?.min_rssi ?? null, unit: 'dBm', better: 'higher' },
		{
			key: 'wifi.dropsWithoutRoam',
			get: (s) => s.summary?.drops_without_roam ?? null,
			unit: '',
			better: 'lower'
		}
	];
	const fmt = (v: number | null) =>
		v == null ? '—' : Number.isInteger(v) ? String(v) : v.toFixed(2);
	function delta(r: Row, a: Survey, b: Survey): { text: string; cls: string } {
		const x = r.get(a);
		const y = r.get(b);
		if (x == null || y == null || x === y) return { text: '±0', cls: 'text-slate-500' };
		const d = y - x;
		const good = r.better === 'lower' ? d < 0 : d > 0;
		return {
			text: `${d > 0 ? '+' : ''}${fmt(d)}`,
			cls: good ? 'text-emerald-700 dark:text-emerald-400' : 'text-red-700 dark:text-red-400'
		};
	}
</script>

<div class="space-y-4">
	<PageHeader
		title={t('wifi.compareTitle')}
		back={{ href: '/wifi-surveys', label: t('wifi.title') }}
	/>
	<ErrorBox {error} />
	{#if data}
		{@const { a, b } = data}
		<section class="card scroll-x">
			<table class="table">
				<thead>
					<tr>
						<th></th>
						<th
							>A · <a class="hover:underline" href={`/wifi-surveys/${a.id}`}
								>{a.label || formatDateTime(a.created_at)}</a
							></th
						>
						<th
							>B · <a class="hover:underline" href={`/wifi-surveys/${b.id}`}
								>{b.label || formatDateTime(b.created_at)}</a
							></th
						>
						<th>B − A</th>
					</tr>
				</thead>
				<tbody>
					<tr>
						<td>{t('wifi.col.device')}</td>
						<td>{a.device_label}</td>
						<td>{b.device_label}</td>
						<td></td>
					</tr>
					<tr>
						<td>{t('wifi.col.duration')}</td>
						<td>{a.summary ? clock(a.summary.duration_ms) : '—'}</td>
						<td>{b.summary ? clock(b.summary.duration_ms) : '—'}</td>
						<td></td>
					</tr>
					{#each rows as r (r.key)}
						{@const d = delta(r, a, b)}
						<tr>
							<td>{t(r.key)}</td>
							<td class="tabular-nums">{fmt(r.get(a))} {r.unit}</td>
							<td class="tabular-nums">{fmt(r.get(b))} {r.unit}</td>
							<td class="tabular-nums {d.cls}">{d.text}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</section>
		<section class="card">
			<h2 class="p-4 pb-0">{t('wifi.comparePairs')}</h2>
			<div class="scroll-x">
				<table class="table">
					<thead>
						<tr>
							<th>{t('wifi.col.fromTo')}</th>
							<th>A</th>
							<th>B</th>
						</tr>
					</thead>
					<tbody>
						{#if !data.pairs.length}<EmptyRow cols={3} text={t('wifi.noRoams')} />{/if}
						{#each data.pairs as p (p.from_ap + p.to_ap)}
							<tr>
								<td>{p.from_ap} ↔ {p.to_ap}</td>
								<td>
									{#if p.a_count}
										{p.a_count}× · Ø {p.a_avg_gap_ms} ms
										<span class={ratingBadge(p.a_worst)}
											>{t(`wifi.rating.${p.a_worst ?? 'clean'}`)}</span
										>
									{:else}—{/if}
								</td>
								<td>
									{#if p.b_count}
										{p.b_count}× · Ø {p.b_avg_gap_ms} ms
										<span class={ratingBadge(p.b_worst)}
											>{t(`wifi.rating.${p.b_worst ?? 'clean'}`)}</span
										>
									{:else}—{/if}
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		</section>
	{/if}
</div>
