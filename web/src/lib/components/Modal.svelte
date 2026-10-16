<script lang="ts">
	import type { Snippet } from 'svelte';
	import { fade, fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { motion } from '#lib/motion.ts';

	let {
		title,
		open = $bindable(false),
		children
	}: { title: string; open?: boolean; children: Snippet } = $props();
</script>

<!-- Escape closes the dialog wherever the focus is. -->
<svelte:window onkeydown={(e) => open && e.key === 'Escape' && (open = false)} />

{#if open}
	<div
		class="fixed inset-0 z-40 flex items-end justify-center bg-black/40 p-0 backdrop-blur-[1px] sm:items-center sm:p-4"
		role="presentation"
		transition:fade={{ duration: motion(180) }}
		onclick={(e) => e.target === e.currentTarget && (open = false)}
	>
		<div
			class="dialog-panel w-full rounded-t-xl sm:max-w-lg sm:rounded-xl"
			transition:fly={{ y: 12, duration: motion(220), easing: cubicOut }}
			role="dialog"
			aria-modal="true"
			aria-label={title}
		>
			<div class="mb-4 flex items-center justify-between gap-3">
				<h2>{title}</h2>
				<button class="btn btn-sm" onclick={() => (open = false)} aria-label={t('common.close')}
					>✕</button
				>
			</div>
			{@render children()}
		</div>
	</div>
{/if}
