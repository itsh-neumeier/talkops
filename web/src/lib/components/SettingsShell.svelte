<script lang="ts">
	import type { Snippet } from 'svelte';
	import SettingsNav from '#lib/components/SettingsNav.svelte';

	let { children }: { children: Snippet } = $props();
</script>

<!-- Setup pages (trunks, numbers, ...) shown with the settings navigation. -->
<div class="flex flex-col gap-4 md:flex-row md:items-start">
	<SettingsNav />
	<div class="min-w-0 flex-1">{@render children()}</div>
</div>
