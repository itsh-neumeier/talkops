<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		title,
		heading,
		back,
		intro,
		actions
	}: {
		/** Plain page title. */
		title?: string;
		/** Rich title (badges, numbers); used instead of `title`. */
		heading?: Snippet;
		/** Link back to the parent page, shown above the title. */
		back?: { href: string; label: string };
		/** Short explanation below the title. */
		intro?: string;
		/** Buttons on the right; they wrap below the title on narrow screens. */
		actions?: Snippet;
	} = $props();
</script>

<!-- Common head of every page: back link, title, actions and intro. -->
<div class="space-y-1">
	{#if back}
		<a
			class="inline-block text-sm text-slate-500 hover:text-slate-900 hover:underline dark:text-slate-400 dark:hover:text-slate-100"
			href={back.href}>← {back.label}</a
		>
	{/if}
	{#if heading || title || actions}
		<div class="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
			{#if heading || title}
				<h1 class="min-w-0 break-words">
					{#if heading}{@render heading()}{:else}{title}{/if}
				</h1>
			{/if}
			{#if actions}
				<div class="flex flex-wrap items-center gap-2">{@render actions()}</div>
			{/if}
		</div>
	{/if}
	{#if intro}
		<p class="text-sm text-slate-600 dark:text-slate-300">{intro}</p>
	{/if}
</div>
