<script lang="ts" module>
	/** Sub-tabs of Settings → System (`?section=system&tab=…`). */
	export const systemTabs = ['status', 'diagnostics', 'sessions', 'maintenance'] as const;
	export type SystemTab = (typeof systemTabs)[number];

	export function isSystemTab(id: string | null | undefined): id is SystemTab {
		return (systemTabs as readonly string[]).includes(id ?? '');
	}
</script>

<script lang="ts">
	import DiagnosticsCapture from '#lib/components/DiagnosticsCapture.svelte';
	import SystemMaintenance from '#lib/components/SystemMaintenance.svelte';
	import SystemSessions from '#lib/components/SystemSessions.svelte';
	import SystemStatus from '#lib/components/SystemStatus.svelte';
	import Tabs from '#lib/components/Tabs.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';

	let {
		tab = $bindable('status'),
		onchange
	}: {
		tab?: SystemTab;
		onchange?: (id: SystemTab) => void;
	} = $props();

	const tabs = $derived(systemTabs.map((id) => ({ id, label: t(`system.tab.${id}`) })));
</script>

<Tabs {tabs} bind:value={tab} {onchange} label={t('settings.section.system')} />

<div class="space-y-4" role="tabpanel" aria-label={t(`system.tab.${tab}`)}>
	{#if tab === 'status'}
		<SystemStatus />
	{:else if tab === 'diagnostics'}
		<DiagnosticsCapture />
	{:else if tab === 'sessions'}
		<SystemSessions />
	{:else}
		<SystemMaintenance />
	{/if}
</div>
