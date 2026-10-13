<script lang="ts">
	import { page } from '$app/state';
	import { t } from '#lib/i18n/index.svelte.ts';
</script>

<!-- Shown inside the normal layout for unknown pages and unexpected errors. -->
<div class="card mx-auto mt-10 max-w-md space-y-3 text-center">
	<p class="text-4xl font-semibold text-teal-700 dark:text-teal-300">{page.status}</p>
	<h1>{page.status === 404 ? t('error.notFound') : t('error.generic')}</h1>
	<p class="text-sm text-slate-600 dark:text-slate-300">
		{page.status === 404 ? t('error.notFoundHint') : t('error.genericHint')}
	</p>
	{#if page.status !== 404 && page.error?.message}
		<p class="font-mono text-xs break-words text-slate-500 dark:text-slate-400">
			{page.error.message}
		</p>
	{/if}
	<div class="flex flex-wrap justify-center gap-2 pt-1">
		<a class="btn btn-primary" href="/">{t('error.home')}</a>
		<button class="btn" onclick={() => location.reload()}>{t('error.reload')}</button>
	</div>
</div>
