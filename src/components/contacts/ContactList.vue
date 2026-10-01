<script lang="ts" setup>
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import {
	ActionButton,
	EmptyState,
	ListRow,
	LoadingState,
	Panel,
	SectionHeading,
} from '@vasakgroup/vue-libvasak';
import { computed } from 'vue';
import { type Contact, initialOf } from '@/tools/address-book';
import { interpolar } from '@/tools/interpolar';
import { NARROW_ONLY } from '@/tools/narrow-layout';

const props = defineProps<{
	contacts: Contact[];
	selected: Contact | null;
	query: string;
	loading: boolean;
	/**
	 * Cuántos hay, cuando no entra en la barra (ventana angosta): va al lado
	 * del botón de volver. Vacío, no se muestra.
	 */
	countLabel?: string;
	/** Con la ventana angosta, los nombres largos se parten en vez de cortarse. */
	wrap?: boolean;
}>();
const emit = defineEmits<{
	select: [contact: Contact];
	/** Volver a las cuentas, con la ventana angosta. */
	back: [];
}>();

const { t } = useI18n();

/**
 * Los contactos agrupados por su inicial.
 *
 * Los grupos son lo que hace recorrible una lista de mil nombres: sin ellos hay
 * que leer renglón por renglón para saber por dónde va uno.
 *
 * Se arma sobre la lista **ya ordenada y ya filtrada**: agrupar antes de
 * filtrar dejaría letras con el encabezado puesto y nadie debajo.
 */
const groups = computed(() => {
	const out: { initial: string; contacts: Contact[] }[] = [];
	for (const contact of props.contacts) {
		const initial = initialOf(contact);
		const last = out.at(-1);
		if (last?.initial === initial) {
			last.contacts.push(contact);
		} else {
			out.push({ initial, contacts: [contact] });
		}
	}
	return out;
});

/** El `#` no es una letra: se dice con palabras. */
function titleOf(initial: string): string {
	return initial === '#' ? t('lista.otros') : initial;
}
</script>

<template>
  <Panel padding="none" scroll class="w-72 max-w-[35%] shrink-0">
    <!-- El ancho es el de siempre (`w-72`) mientras haya lugar, pero nunca más
         del 35 % de la fila, por lo mismo que el panel de cuentas: desde unos
         830 px de fila mide igual que antes, y por debajo le deja lugar a la
         ficha en vez de taparla. -->

    <!-- Con la ventana angosta, una columna por vez: de la lista se vuelve a
         las cuentas. Con la ventana ancha no existe. -->
    <div class="flex items-center justify-between gap-2 p-1" :class="NARROW_ONLY">
      <ActionButton
        variant="ghost"
        size="sm"
        icon="go-previous"
        icon-type="symbol"
        :label="t('nav.accounts')"
        v-bind="{ 'data-nav': '' }"
        @click="emit('back')" />
      <span
        v-if="countLabel"
        class="min-w-0 px-2 text-right text-tx-muted text-xs tabular-nums"
        aria-live="polite"
        data-testid="count">{{ countLabel }}</span>
    </div>
    <LoadingState v-if="loading && contacts.length === 0" size="sm" :label="t('lista.cargando')" />

    <!-- Una búsqueda sin resultados dice **qué** no se encontró. «No hay nada»
         a secas deja a la persona sin saber si escribió mal o si de verdad no
         tiene a nadie anotado. -->
    <EmptyState
      v-else-if="contacts.length === 0 && query.trim()"
      size="sm"
      icon=""
      :title="interpolar(t('lista.sinResultados'), query)" />
    <EmptyState v-else-if="contacts.length === 0" size="sm" icon="" :title="t('lista.vacia')" />

    <template v-else>
      <section v-for="group in groups" :key="group.initial">
        <!-- Pegado arriba al desplazar, con el fondo opaco del panel: la copia
             de acá llevaba `bg-ui-bg/95`, el fondo de la ventana encima del
             panel, y el contenido se transparentaba por debajo. -->
        <SectionHeading :title="titleOf(group.initial)" as="h3" sticky surface="panel" class="px-3" />
        <ul class="flex flex-col px-1">
          <li v-for="contact in group.contacts" :key="contact.url || contact.uid">
            <ListRow
              role="button"
              :truncate="!wrap"
              :title="contact.name || t('lista.sinNombre')"
              :description="contact.organization || undefined"
              :selected="contact.url === selected?.url"
              @click="emit('select', contact)" />
          </li>
        </ul>
      </section>
    </template>
  </Panel>
</template>
