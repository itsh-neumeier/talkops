<script lang="ts">
	import type { Snippet } from 'svelte';
	import SettingsShell from '#lib/components/SettingsShell.svelte';

	let { children }: { children: Snippet } = $props();
</script>

<SettingsShell>{@render children()}</SettingsShell>
