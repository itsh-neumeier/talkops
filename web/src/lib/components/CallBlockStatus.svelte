<script lang="ts">
	import type { Call } from '#lib/api.ts';
	import { allowNumber, blockSource, isBlocked } from '#lib/calls/blocking.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import AllowNumberDialog from './AllowNumberDialog.svelte';
	let { call, onallowed = () => {} }: { call: Call; onallowed?: (number: string) => void } =
		$props();
	let open = $state(false);
	const source = $derived(blockSource(call.blocked_reason ?? ''));
</script>

{#if isBlocked(call)}
	<div class="space-y-1 text-sm" data-testid="blocked-call">
		<span class="badge badge-warn inline-flex items-center gap-1">
			<svg
				aria-hidden="true"
				viewBox="0 0 24 24"
				class="h-3.5 w-3.5 shrink-0"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				><path d="M12 3 3 7v5c0 5 9 9 9 9s9-4 9-9V7z" /><path d="m8 12 3 3 5-6" /></svg
			>
			{t('calls.result.blocked')} · {t(`block.source.${source}`)}
		</span>
		<details class="text-xs text-slate-500 dark:text-slate-400">
			<summary class="cursor-pointer">{t('block.details')}</summary>
			<p class="break-all">
				{source === 'list'
					? t('block.match', { pattern: call.blocked_reason!.slice(5) })
					: source === 'phoneblock'
						? t('block.votes', { count: call.blocked_reason!.slice(11) })
						: t(`block.source.${source}`)}
			</p>
			<p class="break-all font-mono">{call.hangup_cause}</p>
		</details>
		{#if can('blocking.allowlist') && source !== 'anonymous' && allowNumber(call.caller_number)}
			{#if call.allowlisted}<span class="badge badge-ok" role="status">{t('block.allowed')}</span>
			{:else}<button class="btn btn-sm max-w-full text-left" onclick={() => (open = true)}
					>{t('block.allowAdd')}</button
				>{/if}
			<AllowNumberDialog number={call.caller_number} bind:open {onallowed} />
		{/if}
	</div>
{/if}
