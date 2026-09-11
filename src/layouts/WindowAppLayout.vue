<script lang="ts" setup>
import TopBarComponent from '@/components/topbar/TopBarComponent.vue';
</script>
<template>
  <div
    class="flex h-screen w-screen flex-col overflow-hidden rounded-corner-window border border-ui-border bg-ui-bg/80">
    <!-- La barra lleva contenido de la aplicación: el icono, el buscador y los
         botones que valen para toda la ventana. Sin estos `slot` la barra queda
         con los controles de la ventana y nada más, que es como estaba. -->
    <TopBarComponent>
      <slot name="barra" />
      <template v-if="$slots.barraCentro" #centro>
        <slot name="barraCentro" />
      </template>
    </TopBarComponent>
    <!-- El `slot` es lo que hace usable este layout.
         Sin él, `<WindowAppLayout>…</WindowAppLayout>` descartaba en silencio todo
         lo que se le pusiera dentro y la ventana abría vacía con el relleno de la
         plantilla todavía puesto. En vasak-monitor costó una compilación y una
         captura darse cuenta, porque no hay ningún error: simplemente no aparece
         nada. -->
    <div class="flex min-h-0 flex-1 gap-1 p-1">
      <slot>
        <p class="p-4 text-tx-muted text-sm">
          Poné el contenido de la aplicación dentro de
          <code>&lt;WindowAppLayout&gt;</code>.
        </p>
      </slot>
    </div>
  </div>
</template>
