<script lang="ts" setup>
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { computed, onMounted } from 'vue';
import CuentasComponent from '@/components/agenda/CuentasComponent.vue';
import DetalleComponent from '@/components/agenda/DetalleComponent.vue';
import ListaComponent from '@/components/agenda/ListaComponent.vue';
import { useAgenda } from '@/composables/use-agenda';
import { claveSegunCantidad, interpolar } from '@/tools/interpolar';

const { t, locale } = useI18n();
const { cuentas, visibles, elegido, consulta, cargando, avisos, cargar, elegir } = useAgenda(
	() => locale.value
);

const cuantos = computed(() =>
	interpolar(t(claveSegunCantidad('lista.cuantos', visibles.value.length)), visibles.value.length)
);

onMounted(cargar);
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <header class="flex items-center gap-2 border-ui-border border-b px-3 py-2">
      <h1 class="font-title text-lg">{{ t('app.nombre') }}</h1>

      <!-- El buscador arriba de todo y siempre a la vista: es lo que se usa en
           una agenda, y esconderlo detrás de un atajo o un botón lo vuelve
           invisible para quien no lo conoce. -->
      <input
        v-model="consulta"
        type="search"
        class="w-64 rounded-corner-sm border border-ui-border-strong bg-ui-surface/40 px-2 py-1 text-sm"
        :placeholder="t('lista.buscar')"
        :aria-label="t('lista.buscar')" />
      <span class="text-tx-muted text-xs" aria-live="polite">{{ cuantos }}</span>

      <span class="flex-1"></span>

      <span v-if="cargando" class="text-tx-muted text-xs" role="status">
        {{ t('lista.cargando') }}
      </span>
      <button
        v-else
        type="button"
        class="rounded-corner px-2 py-0.5 text-sm text-tx-muted hover:bg-ui-surface"
        @click="cargar()">
        {{ t('lista.actualizar') }}
      </button>
    </header>

    <div class="flex min-h-0 flex-1">
      <CuentasComponent :cuentas="cuentas" :avisos="avisos" />
      <ListaComponent
        :contactos="visibles"
        :elegido="elegido"
        :consulta="consulta"
        :cargando="cargando"
        @elegir="elegir" />
      <DetalleComponent :contacto="elegido" />
    </div>
  </div>
</template>
