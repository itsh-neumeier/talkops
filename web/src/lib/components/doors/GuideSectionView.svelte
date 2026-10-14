<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';
	import type { GuideSection, GuideText } from '#lib/doors/guide.ts';

	let {
		section,
		params,
		open = false
	}: {
		section: GuideSection;
		/** Values for `{host}`, `{port}` and `{domain}` in the texts. */
		params: Record<string, string | number>;
		open?: boolean;
	} = $props();

	const text = (x: GuideText) => (typeof x === 'string' ? t(x, params) : x.raw);
</script>

<details class="card group" {open}>
	<summary class="flex cursor-pointer list-none items-center justify-between gap-2">
		<h3 class="text-base font-semibold">{t(section.title)}</h3>
		<span class="text-slate-400 transition-transform group-open:rotate-90" aria-hidden="true"
			>›</span
		>
	</summary>
	<div class="mt-3 space-y-3 text-sm">
		{#if section.intro}<p>{text(section.intro)}</p>{/if}
		{#if section.steps}
			<ol class="list-decimal space-y-1 pl-5">
				{#each section.steps as step (step)}<li>{text(step)}</li>{/each}
			</ol>
		{/if}
		{#if section.table}
			{#if section.table.caption}<p class="font-medium">{text(section.table.caption)}</p>{/if}
			<div class="scroll-x">
				<table class="w-full text-left text-sm">
					<thead>
						<tr class="border-b border-slate-200 dark:border-slate-700">
							{#each section.table.columns as col (col)}
								<th class="py-1 pr-3 font-medium whitespace-nowrap">{t(col)}</th>
							{/each}
						</tr>
					</thead>
					<tbody>
						{#each section.table.rows as row, i (i)}
							<tr class="border-b border-slate-100 align-top dark:border-slate-800">
								{#each row as cell, j (j)}
									<!-- Device field names are literal: monospace. -->
									<td
										class="py-1 pr-3 {j === 0 ? 'min-w-24' : 'min-w-36'} {typeof cell !== 'string'
											? 'font-mono text-xs'
											: ''}">{text(cell)}</td
									>
								{/each}
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
		{#each section.notes ?? [] as note (note)}
			<p class="hint">{text(note)}</p>
		{/each}
		{#each section.unconfirmed ?? [] as note (note)}
			<p
				class="rounded-lg border border-amber-300 bg-amber-50 p-2 text-amber-900 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-100"
			>
				<span class="badge badge-warn mr-1">{t('doorGuide.unconfirmed')}</span>{text(note)}
			</p>
		{/each}
	</div>
</details>
