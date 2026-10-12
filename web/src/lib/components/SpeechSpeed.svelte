<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';
	let { value = $bindable(1), id }: { value?: number; id: string } = $props();
</script>

<div class="space-y-1">
	<label for={id}>{t('audio.speechSpeed')} · {value.toFixed(2)}×</label>
	<input
		{id}
		class="w-full accent-teal-600"
		type="range"
		min="0.5"
		max="2"
		step="0.05"
		bind:value
	/>
	<p class="hint">{t('audio.speedHint')}</p>
</div>
