<script lang="ts">
	import PhoneNumber from './PhoneNumber.svelte';
	import type { CallParty } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { tick } from 'svelte';
	let { party, flag = false }: { party: CallParty; flag?: boolean } = $props();

	const cardId = $props.id();
	const matches = $derived(party.ambiguous ? (party.matches ?? []) : []);
	const href = (contact: string) => `/phonebook?contact=${encodeURIComponent(contact)}`;
	const showNumber = $derived(!!party.number && party.number !== 'anonymous');

	let open = $state(false);
	let root = $state<HTMLElement>();
	let trigger = $state<HTMLButtonElement>();
	let card = $state<HTMLElement>();
	let place = $state({ top: 0, left: 0 });
	/** Opened by a click or tap: stays open when the pointer leaves. */
	let pinned = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;
	const linkClass =
		'font-medium [overflow-wrap:anywhere] decoration-slate-300 decoration-dotted underline-offset-2 hover:text-teal-700 hover:underline focus-visible:underline dark:decoration-slate-600 dark:hover:text-teal-300';

	/** Places the card below the trigger, or above it near the bottom edge. */
	function position() {
		if (!trigger || !card) return;
		const at = trigger.getBoundingClientRect();
		const gap = 6;
		const margin = 8;
		const below = at.bottom + gap;
		const top =
			below + card.offsetHeight > innerHeight - margin && at.top - gap - card.offsetHeight > margin
				? at.top - gap - card.offsetHeight
				: below;
		const left = Math.min(
			Math.max(margin, at.left),
			Math.max(margin, innerWidth - card.offsetWidth - margin)
		);
		place = { top, left };
	}

	async function show() {
		clearTimeout(timer);
		if (open) return;
		open = true;
		await tick();
		// Top layer: dialogs and scroll areas cannot clip the card.
		if (card && 'showPopover' in card && !card.matches(':popover-open')) card.showPopover();
		position();
	}

	function hide() {
		clearTimeout(timer);
		if (card && 'hidePopover' in card && card.matches(':popover-open')) card.hidePopover();
		open = false;
		pinned = false;
	}

	// Hover opens after a short delay and closes after a grace period, so the
	// pointer can move from the trigger into the card.
	const later = (action: () => void, ms: number) => {
		clearTimeout(timer);
		timer = setTimeout(action, ms);
	};

	function outside(event: PointerEvent) {
		if (root && !root.contains(event.target as Node)) hide();
	}

	$effect(() => {
		if (!open) return;
		document.addEventListener('pointerdown', outside);
		addEventListener('scroll', position, true);
		addEventListener('resize', position);
		return () => {
			document.removeEventListener('pointerdown', outside);
			removeEventListener('scroll', position, true);
			removeEventListener('resize', position);
		};
	});
	$effect(() => () => clearTimeout(timer));
</script>

{#snippet number()}
	{#if showNumber}
		<span class="inline-block max-w-full break-all text-slate-500 dark:text-slate-400"
			><PhoneNumber number={party.number} {flag} internal={!!party.extension_id} brackets /></span
		>
	{:else}
		<span>{t('dash.party.anonymous')}</span>
	{/if}
{/snippet}

{#if matches.length > 1}
	<!-- Hover and Escape only add to the button, which opens the card by keyboard and touch. -->
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<span
		bind:this={root}
		class="inline"
		role="group"
		onmouseenter={() => later(show, 250)}
		onmouseleave={() => !pinned && later(hide, 200)}
		onfocusout={(e) => {
			if (!root?.contains(e.relatedTarget as Node)) hide();
		}}
		onkeydown={(e) => {
			if (e.key === 'Escape' && open) {
				e.stopPropagation();
				hide();
				trigger?.focus();
			}
		}}
	>
		{#if party.name && party.contact_id}
			<a
				class={linkClass}
				href={href(party.contact_id)}
				title={t('dash.party.openEntry', { name: party.name })}>{party.name}</a
			>{' '}
		{/if}
		{@render number()}
		<button
			bind:this={trigger}
			type="button"
			class="ml-1 rounded-full px-1.5 text-xs font-medium whitespace-nowrap text-teal-700 ring-1 ring-teal-200 hover:bg-teal-50 focus-visible:outline-2 focus-visible:outline-teal-500 dark:text-teal-300 dark:ring-teal-800 dark:hover:bg-teal-950"
			aria-expanded={open}
			aria-controls={cardId}
			aria-label={t('dash.party.moreLabel', { count: matches.length })}
			onclick={() => {
				if (open && pinned) return hide();
				pinned = true;
				show();
			}}>{t('dash.party.more', { count: matches.length - 1 })}</button
		>
		{#if open}
			<div
				bind:this={card}
				id={cardId}
				popover="manual"
				class="fixed inset-auto z-[60] m-0 w-max max-w-[min(32rem,calc(100vw-1rem))] rounded-xl border border-slate-200 bg-white p-3 text-slate-900 shadow-lg transition-opacity duration-150 motion-reduce:transition-none dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100 starting:opacity-0"
				style="top: {place.top}px; left: {place.left}px"
				role="region"
				aria-label={t('dash.party.entriesFor', { number: party.number })}
				onmouseenter={() => clearTimeout(timer)}
				onmouseleave={() => !pinned && later(hide, 200)}
			>
				<p class="mb-2 text-xs font-medium text-slate-500 dark:text-slate-400">
					{t('dash.party.entriesFor', { number: `[${party.number}]` })}
				</p>
				<div class="scroll-x">
					<table class="w-full text-left text-sm">
						<thead class="text-xs text-slate-500 max-sm:sr-only dark:text-slate-400">
							<tr>
								<th class="py-1 pr-3 font-medium max-sm:hidden">{t('dash.party.phonebook')}</th>
								<th class="py-1 pr-3 font-medium">{t('dash.party.entry')}</th>
								<th class="py-1"><span class="sr-only">{t('dash.party.goTo')}</span></th>
							</tr>
						</thead>
						<tbody class="divide-y divide-slate-100 dark:divide-slate-800">
							{#each matches as m (m.contact_id)}
								{@const current = m.contact_id === party.contact_id}
								<tr class={current ? 'bg-teal-50/70 dark:bg-teal-950/40' : ''}>
									<td class="py-1.5 pr-3 text-slate-600 max-sm:hidden dark:text-slate-300">
										{m.section_id ? m.section_name : t('contacts.global')}
									</td>
									<td class="py-1.5 pr-3 [overflow-wrap:anywhere]">
										<span class="block text-xs text-slate-500 sm:hidden dark:text-slate-400"
											>{m.section_id ? m.section_name : t('contacts.global')}</span
										>
										<span class="font-medium">{m.name}</span>
										{#if current}
											<span class="badge-muted ml-1 align-middle whitespace-nowrap"
												>{t('dash.party.shown')}</span
											>
										{/if}
									</td>
									<td class="py-1.5 text-right whitespace-nowrap">
										<a class="btn btn-sm" href={href(m.contact_id)} onclick={hide}
											>{t('dash.party.goTo')} <span aria-hidden="true">→</span></a
										>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</div>
		{/if}
	</span>
{:else}
	{#if party.name && party.contact_id}
		<a
			class={linkClass}
			href={href(party.contact_id)}
			title={t('dash.party.openEntry', { name: party.name })}>{party.name}</a
		>
	{:else if party.name}
		<strong class="font-medium [overflow-wrap:anywhere]">{party.name}</strong>
	{/if}
	{#if party.name}{' '}{/if}
	{@render number()}
{/if}
