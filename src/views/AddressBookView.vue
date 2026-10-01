<script lang="ts" setup>
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { ActionButton, BarSearch, ThemeIcon } from '@vasakgroup/vue-libvasak';
import { computed, nextTick, onMounted, ref, useTemplateRef } from 'vue';
import AccountsPanel from '@/components/contacts/AccountsPanel.vue';
import ContactDetail from '@/components/contacts/ContactDetail.vue';
import ContactList from '@/components/contacts/ContactList.vue';
import { useAgenda } from '@/composables/use-agenda';
import { useNarrowRow } from '@/composables/use-narrow-row';
import WindowAppLayout from '@/layouts/WindowAppLayout.vue';
import type { Contact } from '@/tools/address-book';
import { claveSegunCantidad, interpolar } from '@/tools/interpolar';
import { type Pane, paneClass } from '@/tools/narrow-layout';

const { t, locale } = useI18n();
const { cuentas, visibles, elegido, consulta, cargando, avisos, cargar, elegir } = useAgenda(
	() => locale.value
);

const countLabel = computed(() =>
	interpolar(t(claveSegunCantidad('lista.cuantos', visibles.value.length)), visibles.value.length)
);

/**
 * La columna que se mira con la ventana angosta (`tools/narrow-layout.ts`).
 * Se arranca por la lista, que es lo que se viene a buscar; las cuentas
 * quedan un paso atrás.
 */
const pane = ref<Pane>('list');

/**
 * Si la fila está angosta, para lo que no alcanza con las clases: la barra no
 * está adentro de la fila. Ahí el buscador se pliega en la lupa —un campo de 70
 * px se cortaba en «Bu»— y la cuenta de contactos baja a la lista, donde entra.
 */
const layout = useTemplateRef<InstanceType<typeof WindowAppLayout>>('layout');
const { narrow } = useNarrowRow(() => layout.value?.row);

/**
 * Pasa a otra columna y le lleva el foco a su botón de ir o volver: la que se
 * deja se oculta, y un foco en algo oculto se pierde. Con la ventana ancha ese
 * botón no se muestra y el foco se queda donde estaba.
 */
async function go(next: Pane) {
	pane.value = next;
	await nextTick();
	const target = document.querySelector<HTMLElement>(`[data-pane="${next}"] [data-nav]`);
	target?.focus();
}

/** Elegir a alguien es, además, ir a su ficha. */
function select(contact: Contact) {
	elegir(contact);
	void go('detail');
}

onMounted(cargar);
</script>

<template>
  <WindowAppLayout ref="layout">
    <!-- El icono de la aplicación, a la izquierda de todo, como en el resto
         del escritorio. Reemplaza al título escrito: el nombre de la ventana ya
         lo dice el icono, y el renglón que ocupaba era el que empujaba al
         buscador a un costado.

         Va en `identidad` y no en el contenido de la barra: es la única zona
         que no se desplaza con el resto cuando la barra queda a un costado. -->
    <template #identidad>
      <!-- A color y no monocromo: es la identidad de la ventana, como en el
           resto del escritorio.

           `contacts` y no `x-office-address-book`, que es el nombre más obvio:
           ese otro existe **también** como icono de tipo de archivo, y el tema
           resolvía el de `mimes/` — una hoja con una arroba, que no es la
           aplicación. Éste está sólo en `apps/`, así que no hay qué desempatar. -->
      <ThemeIcon name="contacts" :size="24" :alt="t('app.nombre')" />
    </template>

    <!-- El estado y el botón de actualizar, junto a los botones de la ventana,
         que es donde están en el resto de las aplicaciones. El hueco que los
         empujaba hasta ahí —un `span` con `flex-1`— lo pone la barra sola. -->
    <template #acciones>
      <!-- El estado de carga se dice, no se insinúa con un icono girando: sin
           esto, un servidor lento y una agenda vacía se ven igual. -->
      <span v-if="cargando" class="text-tx-muted text-xs" role="status">
        {{ t('lista.cargando') }}
      </span>
      <!-- `ghost`, como los tres botones de la ventana que tiene al lado: los
           cuatro son controles de la barra y se leen como un grupo. -->
      <ActionButton
        variant="ghost"
        label=""
        icon="view-refresh"
        :icon-alt="t('lista.actualizar')"
        :title="t('lista.actualizar')"
        :disabled="cargando"
        @click="cargar()" />
    </template>

    <!-- El buscador siempre a la vista: es lo que se usa en una agenda, y
         esconderlo detrás de un atajo o un botón lo vuelve invisible para quien
         no lo conoce.

         **Centrado en el hueco que queda**, no en la ventana entera. Va en el
         contenido de la barra —la única ranura que crece— con `m-auto`, que en
         un contenedor flexible reparte lo que sobra a los dos lados. Estaba en
         `centro`, que centra respecto de la ventana: con el icono de un lado y
         el estado, el botón de actualizar y los tres controles del otro, el
         medio de la ventana no es el medio del hueco, y el campo quedaba
         corrido a la derecha con la mitad izquierda de la barra vacía.

         `m-auto` y no `mx-auto` porque la barra también puede ir a un costado:
         ahí el eje del hueco es el vertical, y el margen automático en los dos
         ejes centra en el que corresponda sin preguntar cuál es.

         Es el `BarSearch` de la librería, que resuelve el único caso donde
         «siempre a la vista» no se puede cumplir: con la barra a un costado hay
         cuarenta y ocho píxeles de ancho y un campo de texto ahí no se lee ni
         se escribe. Ahí queda la lupa y el campo se abre al lado de la barra. -->
    <template #barra>
      <div class="m-auto flex items-center gap-2">
        <BarSearch
          v-model="consulta"
          :collapsed="narrow"
          :placeholder="t('lista.buscar')"
          :label="t('lista.buscar')" />
        <!-- Cuántos hay, que con una búsqueda escrita es cuántos coinciden. Es
             la única respuesta que da el buscador cuando no encuentra nada.

             Al lado del campo y contando para el centrado: lo que se ve como
             una sola cosa es «campo más número», y centrar sólo el campo deja
             al conjunto corrido. Con el ancho mínimo, pasar de «9 contactos» a
             «124 contactos» no mueve el campo; sin él, cada dígito lo corría
             medio carácter, que es por lo que este número colgaba aparte. -->
        <span
          v-if="!narrow"
          class="min-w-24 whitespace-nowrap text-tx-muted text-xs tabular-nums"
          aria-live="polite">{{ countLabel }}</span>
      </div>
    </template>

    <!-- Las secciones separadas por aire y no por líneas: cada una es una
         superficie redondeada, como los paneles del escritorio. -->
    <AccountsPanel
      :accounts="cuentas"
      :notices="avisos"
      v-bind="{ 'data-pane': 'accounts' }"
      :class="paneClass('accounts', pane)"
      @forward="go('list')" />
    <ContactList
      :contacts="visibles"
      :selected="elegido"
      :query="consulta"
      :loading="cargando"
      :count-label="narrow ? countLabel : ''"
      :wrap="narrow"
      v-bind="{ 'data-pane': 'list' }"
      :class="paneClass('list', pane)"
      @select="select"
      @back="go('accounts')" />
    <ContactDetail
      :contact="elegido"
      v-bind="{ 'data-pane': 'detail' }"
      :class="paneClass('detail', pane)"
      @back="go('list')" />
  </WindowAppLayout>
</template>
