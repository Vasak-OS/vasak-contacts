<script lang="ts" setup>
import ActionControlsComponent from '@/components/topbar/ActionControlsComponent.vue';
</script>
<template>
  <div class="relative flex justify-between p-1 font-title" data-tauri-drag-region>
    <div class="flex-1 flex justify-between items-center gap-2 px-1" data-tauri-drag-region>
      <slot />
    </div>
    <ActionControlsComponent />

    <!-- El `centro` va **encima** de la barra y no como una columna más.
         Centrado entre dos columnas queda centrado respecto de lo que sobra a
         los costados, no de la ventana: con el icono a la izquierda y tres
         controles a la derecha, eso lo corre visiblemente. Absoluto y al 50% del
         ancho total queda donde la gente espera.

         `pointer-events-none` en el envoltorio para no tapar la zona de
         arrastre de la ventana; lo que va adentro lo vuelve a encender. -->
    <div
      v-if="$slots.centro"
      class="pointer-events-none absolute inset-y-0 left-1/2 flex -translate-x-1/2 items-center">
      <div class="pointer-events-auto flex items-center gap-2">
        <slot name="centro" />
      </div>
    </div>
  </div>
</template>