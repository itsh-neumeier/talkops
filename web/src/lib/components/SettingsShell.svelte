<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/state';
	import SettingsNav from '#lib/components/SettingsNav.svelte';

	let { children }: { children: Snippet } = $props();

	// The flow editor needs the full width; it has its own way back.
	const wide = $derived(page.url.pathname.startsWith('/routing/attendants/'));
</script>

<!-- Setup pages (extensions, trunks, ...) shown with the settings navigation. -->
{#if wide}
	{@render children()}
{:else}
	<div class="flex flex-col gap-4 lg:flex-row lg:items-start">
		<SettingsNav />
		<div class="@container min-w-0 flex-1">{@render children()}</div>
	</div>
{/if}
