<script lang="ts">
	import type { Snippet } from 'svelte';
	import { t } from '#lib/i18n/index.svelte.ts';

	let {
		title,
		open = $bindable(false),
		children
	}: { title: string; open?: boolean; children: Snippet } = $props();
</script>

{#if open}
	<div
		class="fixed inset-0 z-40 flex items-end justify-center bg-black/40 p-0 sm:items-center sm:p-4"
		role="presentation"
		onclick={(e) => e.target === e.currentTarget && (open = false)}
		onkeydown={(e) => e.key === 'Escape' && (open = false)}
	>
		<div
			class="max-h-[90vh] w-full overflow-y-auto rounded-t-xl bg-white p-4 shadow-xl sm:max-w-lg sm:rounded-xl dark:bg-slate-900"
			role="dialog"
			aria-modal="true"
			aria-label={title}
		>
			<div class="mb-4 flex items-center justify-between">
				<h2>{title}</h2>
				<button class="btn btn-sm" onclick={() => (open = false)} aria-label={t('common.close')}
					>✕</button
				>
			</div>
			{@render children()}
		</div>
	</div>
{/if}
