<script lang="ts">
	// First steps after the setup, checked off from the live configuration.
	// Hidden once everything is done or when the admin dismisses it.
	import { onMount } from 'svelte';
	import { api, type Extension, type PhoneNumber, type Settings, type Trunk } from '#lib/api.ts';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { session } from '#lib/session.svelte.ts';

	let { registrations }: { registrations: number | null } = $props();

	const HIDE_KEY = 'talkops.gettingStarted.hidden';

	type Counts = {
		extensions: number;
		trunks: number;
		routed: number;
		defaultNumber: boolean;
		backup: boolean;
	};
	let counts = $state<Counts | null>(null);
	let hidden = $state(false);

	onMount(async () => {
		try {
			hidden = localStorage.getItem(HIDE_KEY) === '1';
		} catch {
			// storage unavailable: always show
		}
		if (hidden) return;
		try {
			const [extensions, trunks, numbers, settings, backups] = await Promise.all([
				api.get<Extension[]>('/extensions'),
				api.get<Trunk[]>('/trunks'),
				api.get<PhoneNumber[]>('/numbers'),
				api.get<Settings>('/settings'),
				api
					.get<{ settings: { enabled: boolean; last_file: string | null } }>('/backups')
					.catch(() => null)
			]);
			counts = {
				extensions: extensions.length,
				trunks: trunks.length,
				routed: numbers.filter((n) => n.destination_type !== 'none').length,
				defaultNumber: !!settings.default_number_id,
				backup: !!backups?.settings.enabled
			};
		} catch {
			counts = null;
		}
	});

	type Step = { key: MessageKey; hint: MessageKey; href: string; done: boolean };
	const steps = $derived<Step[]>(
		counts
			? [
					{
						key: 'start.extension',
						hint: 'start.extensionHint',
						href: '/extensions',
						done: counts.extensions > 0
					},
					{
						key: 'start.phone',
						hint: 'start.phoneHint',
						href: '/phones',
						done: (registrations ?? 0) > 0
					},
					{
						key: 'start.trunk',
						hint: 'start.trunkHint',
						href: '/trunks',
						done: counts.trunks > 0
					},
					{
						key: 'start.number',
						hint: 'start.numberHint',
						href: '/numbers',
						done: counts.routed > 0
					},
					{
						key: 'start.defaultNumber',
						hint: 'start.defaultNumberHint',
						href: '/settings',
						done: counts.defaultNumber
					},
					{
						key: 'start.twoFactor',
						hint: 'start.twoFactorHint',
						href: '/me',
						done: !!session.user?.totp_enabled || session.user?.auth_source !== 'local'
					},
					{
						key: 'start.backup',
						hint: 'start.backupHint',
						href: '/settings',
						done: counts.backup
					}
				]
			: []
	);
	const remaining = $derived(steps.filter((s) => !s.done).length);

	function hide() {
		hidden = true;
		try {
			localStorage.setItem(HIDE_KEY, '1');
		} catch {
			// ignore
		}
	}
</script>

{#if !hidden && steps.length && remaining > 0}
	<section class="card" aria-labelledby="start-title">
		<div class="mb-3 flex items-start justify-between gap-3">
			<div>
				<h2 id="start-title">{t('start.title')}</h2>
				<p class="hint">
					{t('start.progress', { done: steps.length - remaining, total: steps.length })}
				</p>
			</div>
			<button class="btn btn-sm" type="button" onclick={hide}>{t('start.hide')}</button>
		</div>
		<ol class="divide-y divide-slate-100 dark:divide-slate-800">
			{#each steps as step, i (step.key)}
				<li class="flex items-start gap-3 py-2">
					<span
						class="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-xs font-semibold {step.done
							? 'bg-emerald-600 text-white'
							: 'bg-slate-200 text-slate-700 dark:bg-slate-700 dark:text-slate-200'}"
						aria-hidden="true">{step.done ? '✓' : i + 1}</span
					>
					<div class="min-w-0 flex-1">
						<a
							class="font-medium hover:underline {step.done ? 'text-slate-500 line-through' : ''}"
							href={step.href}>{t(step.key)}</a
						>
						<span class="sr-only">{step.done ? t('start.done') : ''}</span>
						{#if !step.done}<p class="hint">{t(step.hint)}</p>{/if}
					</div>
				</li>
			{/each}
		</ol>
	</section>
{/if}
