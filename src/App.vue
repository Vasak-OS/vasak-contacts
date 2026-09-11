<script setup lang="ts">
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useConfigStore } from '@vasakgroup/plugin-config-manager';
import { onMounted, onUnmounted, type Ref, ref } from 'vue';
import AgendaView from '@/views/AgendaView.vue';

let unListenConfig: Ref<UnlistenFn | null> = ref(null);

// La **primera** lectura de la configuración ya no está acá: la hace `main.ts`
// antes de montar. Estando en `onMounted` corría después del primer dibujo, así
// que hasta que resolvía no existían las variables `--use-*` y la ventana se veía
// unos instantes con el tema claro, que es a lo que cae la hoja de estilo cuando
// no hay ninguna. Lo que queda acá es escuchar los cambios de después.
onMounted(async () => {
	try {
		const configStore = useConfigStore();

		unListenConfig.value = await listen('config-changed', async () => {
			document.startViewTransition(() => {
				configStore.loadConfig();
			});
		});
	} catch (error: any) {
		console.error('Error al escuchar los cambios de configuración en App.vue', error);
	}
});

onUnmounted(() => {
	if (unListenConfig.value !== null) {
		unListenConfig.value();
	}
});
</script>

<template>
  <!-- La vista es dueña de la ventana entera, layout incluido.
       Así el buscador y el botón de actualizar salen del mismo `useAgenda()`
       que la lista, sin duplicar el estado ni teletransportar nada. -->
  <AgendaView />
</template>
