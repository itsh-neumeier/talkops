<script lang="ts">
	import { vendorName } from '#lib/vendors.ts';
	let { vendor }: { vendor?: string } = $props();
	const name = $derived(vendorName(vendor));
</script>

<!-- A text badge instead of a logo: manufacturer logos are protected artwork. -->
{#if name}<span
		class="inline-flex h-6 w-24 shrink-0 items-center justify-center truncate rounded-md border border-slate-300 bg-white px-1.5 text-[11px] font-semibold tracking-wide text-slate-700 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-200"
		title={name}>{name}</span
	>{/if}
