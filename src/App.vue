<script setup lang="ts">
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useConfigStore } from '@vasakgroup/plugin-config-manager';
import { onMounted, onUnmounted, type Ref, ref } from 'vue';
import AgendaView from '@/views/AgendaView.vue';

let unListenConfig: Ref<UnlistenFn | null> = ref(null);

onMounted(async () => {
	try {
		const configStore = useConfigStore();
		await configStore.loadConfig();

		unListenConfig.value = await listen('config-changed', async () => {
			document.startViewTransition(() => {
				configStore.loadConfig();
			});
		});
	} catch (error: any) {
		console.error('Error al cargar configuración en App.vue', error);
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
