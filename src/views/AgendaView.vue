<script lang="ts" setup>
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { computed, onMounted } from 'vue';
import CuentasComponent from '@/components/agenda/CuentasComponent.vue';
import DetalleComponent from '@/components/agenda/DetalleComponent.vue';
import ListaComponent from '@/components/agenda/ListaComponent.vue';
import { useAgenda } from '@/composables/use-agenda';
import { useReactiveIcons } from '@/composables/useReactiveIcon';
import WindowAppLayout from '@/layouts/WindowAppLayout.vue';
import { claveSegunCantidad, interpolar } from '@/tools/interpolar';

const { t, locale } = useI18n();
const { cuentas, visibles, elegido, consulta, cargando, avisos, cargar, elegir } = useAgenda(
	() => locale.value
);

const { actualizar, icono } = useReactiveIcons({
	actualizar: 'view-refresh',
	// El icono de la aplicación, no un símbolo: es la identidad de la ventana y
	// va a color, como en el resto del escritorio.
	//
	// `contacts` y no `x-office-address-book`, que es el nombre más obvio: ese
	// otro existe **también** como icono de tipo de archivo, y el tema resolvía
	// el de `mimes/` — una hoja con una arroba, que no es la aplicación. Este
	// nombre está sólo en `apps/`, así que no hay nada que desempatar.
	icono: { name: 'contacts', type: 'icon' },
});

const cuantos = computed(() =>
	interpolar(t(claveSegunCantidad('lista.cuantos', visibles.value.length)), visibles.value.length)
);

onMounted(cargar);
</script>

<template>
  <WindowAppLayout>
    <template #barra>
      <!-- El icono de la aplicación, a la izquierda de todo, como en el resto
           del escritorio. Reemplaza al título escrito: el nombre de la ventana
           ya lo dice el icono, y el renglón que ocupaba era el que empujaba al
           buscador a un costado. -->
      <img :src="icono" class="h-6 w-6 shrink-0" :alt="t('app.nombre')" />

      <!-- Lo que sigue se va contra los controles de la ventana, que es donde
           está el botón de actualizar en el resto de las aplicaciones. -->
      <span class="flex-1"></span>

      <!-- El estado de carga se dice, no se insinúa con un icono girando: sin
           esto, un servidor lento y una agenda vacía se ven igual. -->
      <span v-if="cargando" class="text-tx-muted text-xs" role="status">
        {{ t('lista.cargando') }}
      </span>
      <button
        type="button"
        class="rounded-corner border border-ui-border bg-ui-bg/80 p-1 hover:bg-ui-surface disabled:opacity-50"
        :aria-label="t('lista.actualizar')"
        :title="t('lista.actualizar')"
        :disabled="cargando"
        @click="cargar()">
        <img :src="actualizar" class="h-6 w-6" alt="" />
      </button>
    </template>

    <!-- El buscador al medio de la barra y siempre a la vista: es lo que se usa
         en una agenda, y esconderlo detrás de un atajo o un botón lo vuelve
         invisible para quien no lo conoce.

         Centrado en la barra entera, no en lo que sobra entre el icono y los
         controles de la ventana. -->
    <template #barraCentro>
      <div class="relative">
        <input
          v-model="consulta"
          type="search"
          class="w-72 rounded-corner border border-ui-border-strong bg-ui-surface px-2 py-1 text-sm text-tx-main focus:border-primary focus:outline-none"
          :placeholder="t('lista.buscar')"
          :aria-label="t('lista.buscar')" />
        <!-- Cuántos hay, que con una búsqueda escrita es cuántos coinciden. Es
             la única respuesta que da el buscador cuando no encuentra nada.

             **Colgado del campo y no al lado**: contando para el centrado, el
             buscador se corre a la izquierda, y además se movería solo al pasar
             de «9 contactos» a «124 contactos». Así el campo queda centrado en
             la barra y el número no lo toca. -->
        <span
          class="pointer-events-none absolute top-1/2 left-full ml-2 -translate-y-1/2 whitespace-nowrap text-tx-muted text-xs"
          aria-live="polite">{{ cuantos }}</span>
      </div>
    </template>

    <!-- Las secciones separadas por aire y no por líneas: cada una es una
         superficie redondeada, como los paneles del escritorio. -->
    <CuentasComponent :cuentas="cuentas" :avisos="avisos" />
    <ListaComponent
      :contactos="visibles"
      :elegido="elegido"
      :consulta="consulta"
      :cargando="cargando"
      @elegir="elegir" />
    <DetalleComponent :contacto="elegido" />
  </WindowAppLayout>
</template>
