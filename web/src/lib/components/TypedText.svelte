<script lang="ts">
	import { onMount } from 'svelte';
	let { text, animate = false }: { text: string; animate?: boolean } = $props();
	let visible = $state('');
	let typing = $state(false);
	onMount(() => {
		visible = text;
		if (!animate || matchMedia('(prefers-reduced-motion: reduce)').matches) return;
		const letters = Array.from(text);
		let offset = 0;
		visible = '';
		typing = true;
		const timer = setInterval(() => {
			offset = Math.min(letters.length, offset + Math.max(1, Math.ceil(letters.length / 60)));
			visible = letters.slice(0, offset).join('');
			if (offset === letters.length) {
				typing = false;
				clearInterval(timer);
			}
		}, 20);
		return () => clearInterval(timer);
	});
</script>

<span class="sr-only">{text}</span><span aria-hidden="true"
	>{visible}{#if typing}<span class="text-teal-600">▏</span>{/if}</span
>
