<script lang="ts">
	// Details of one map bubble: totals for each category and the newest calls.
	import type { Call, MapArea } from '#lib/api.ts';
	import CountryFlag from '#lib/components/CountryFlag.svelte';
	import PhoneNumber from '#lib/components/PhoneNumber.svelte';
	import {
		CATEGORIES,
		CATEGORY_COLORS,
		callCategory,
		listHref,
		type Category,
		type MapRange
	} from '#lib/calls/map.ts';
	import { theme } from '#lib/theme.svelte.ts';
	import { formatDateTime, formatDuration, t } from '#lib/i18n/index.svelte.ts';

	let {
		area,
		range,
		shown,
		phonebook = true,
		onclose
	}: {
		area: MapArea;
		range: MapRange;
		shown: Category[];
		/** Show phone book names instead of numbers. */
		phonebook?: boolean;
		onclose: () => void;
	} = $props();

	let heading: HTMLElement | undefined = $state();

	const title = $derived(area.kind === 'none' ? t('callMap.noLocation') : area.name);
	const place = $derived(`${title} · ${area.prefix}`);

	$effect(() => {
		// Focus the heading when another bubble opens, for screen readers.
		void area.id;
		heading?.focus();
	});

	function external(c: Call) {
		return c.direction === 'outbound'
			? { party: c.called, raw: c.destination }
			: { party: c.caller, raw: c.caller_number };
	}

	function extension(c: Call): string {
		const p = c.direction === 'outbound' ? c.caller : c.called;
		return p?.extension_id ? p.number : '';
	}

	const color = (c: Category) => (theme.dark ? CATEGORY_COLORS[c].dark : CATEGORY_COLORS[c].light);
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Escape') onclose();
	}}
/>

<section class="space-y-3" aria-labelledby="call-map-popup-title">
	<div class="flex items-start justify-between gap-2">
		<div class="min-w-0">
			<h2 id="call-map-popup-title" tabindex="-1" bind:this={heading} class="font-semibold">
				{title}
				<span class="font-mono font-normal text-slate-500 dark:text-slate-400">· {area.prefix}</span
				>
			</h2>
			{#if area.country}
				<div class="text-sm text-slate-600 dark:text-slate-300">
					<CountryFlag country={area.country} />
				</div>
			{/if}
		</div>
		<button
			type="button"
			class="btn btn-sm"
			aria-label={t('common.close')}
			title={t('common.close')}
			onclick={onclose}>✕</button
		>
	</div>

	<dl class="grid grid-cols-2 gap-x-3 gap-y-1 text-sm">
		{#each CATEGORIES.filter((c) => shown.includes(c)) as c (c)}
			<dt class="flex items-center gap-1.5">
				<span class="inline-block h-2.5 w-2.5 rounded-full" style:background={color(c)}></span>
				{t(`callMap.category.${c}`)}
			</dt>
			<dd class="text-right font-mono">{area.counts[c]}</dd>
		{/each}
		<dt class="font-medium">{t('callMap.duration')}</dt>
		<dd class="text-right font-mono">{formatDuration(area.billsec)}</dd>
	</dl>
	{#if area.kind === 'none' && area.mobile + area.service + area.unresolved > 0}
		<p class="text-xs text-slate-500 dark:text-slate-400">
			{t('callMap.noLocationReasons', {
				mobile: area.mobile,
				service: area.service,
				other: area.unresolved
			})}
		</p>
	{/if}

	<ul class="divide-y divide-slate-200 text-sm dark:divide-slate-800">
		{#each area.calls as c (c.id)}
			{@const ext = external(c)}
			{@const cat = callCategory(c)}
			{@const own = extension(c)}
			<li class="py-1.5">
				<div class="flex items-baseline justify-between gap-2">
					<span class="min-w-0 truncate">
						<span aria-hidden="true">{c.direction === 'outbound' ? '↗' : '↙'}</span>
						<span class="sr-only">{t(`dir.${c.direction}`)}</span>
						{#if phonebook && ext.party?.name && ext.party.contact_id}
							<span class="font-medium">{ext.party.name}</span>
						{:else}
							<span class="font-mono"
								><PhoneNumber
									number={ext.raw || '—'}
									countryNumber={ext.party?.number || ext.raw}
								/></span
							>
						{/if}
					</span>
					<span class="shrink-0 text-xs text-slate-500 dark:text-slate-400"
						>{formatDateTime(c.started_at)}</span
					>
				</div>
				<div class="flex flex-wrap items-center gap-x-2 text-xs text-slate-500 dark:text-slate-400">
					<span class="inline-flex items-center gap-1">
						<span class="inline-block h-2 w-2 rounded-full" style:background={color(cat)}></span>
						{t(`callMap.category.${cat}`)}
					</span>
					{#if c.answered_at}<span class="font-mono">{formatDuration(c.billsec)}</span>{/if}
					{#if own}<span>{t('calls.extensionShort')}&nbsp;<span class="font-mono">{own}</span></span
						>{/if}
				</div>
			</li>
		{/each}
	</ul>
	{#if area.total > area.calls.length}
		<p class="text-xs text-slate-500 dark:text-slate-400">
			{t('callMap.moreCalls', { shown: area.calls.length, total: area.total })}
		</p>
	{/if}
	<a class="btn btn-sm" href={listHref(range, area.id, place)}>{t('callMap.showInList')}</a>
</section>
