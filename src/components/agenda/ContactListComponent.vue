<script lang="ts" setup>
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { computed } from 'vue';
import { type Contact, initialOf } from '@/tools/address-book';
import { interpolar } from '@/tools/interpolar';

const props = defineProps<{
	contacts: Contact[];
	selected: Contact | null;
	query: string;
	loading: boolean;
}>();
const emit = defineEmits<{ select: [contact: Contact] }>();

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
  <div class="flex w-72 shrink-0 flex-col overflow-y-auto rounded-corner border border-ui-border bg-ui-surface/45">
    <p v-if="loading && contacts.length === 0" class="p-3 text-tx-muted text-sm" role="status">
      {{ t('lista.cargando') }}
    </p>

    <!-- Una búsqueda sin resultados dice **qué** no se encontró. «No hay nada»
         a secas deja a la persona sin saber si escribió mal o si de verdad no
         tiene a nadie anotado. -->
    <p v-else-if="contacts.length === 0 && query.trim()" class="p-3 text-tx-muted text-sm">
      {{ interpolar(t('lista.sinResultados'), query) }}
    </p>
    <p v-else-if="contacts.length === 0" class="p-3 text-tx-muted text-sm">
      {{ t('lista.vacia') }}
    </p>

    <template v-else>
      <section v-for="group in groups" :key="group.initial">
        <h3
          class="sticky top-0 bg-ui-bg/95 px-3 py-1 font-medium text-tx-muted text-xs uppercase">
          {{ titleOf(group.initial) }}
        </h3>
        <ul>
          <li v-for="contact in group.contacts" :key="contact.url || contact.uid">
            <button
              type="button"
              class="flex w-full flex-col gap-0.5 px-3 py-1.5 text-left hover:bg-ui-surface/60"
              :class="{ 'bg-ui-surface': contact.url === selected?.url }"
              :aria-current="contact.url === selected?.url ? 'true' : undefined"
              @click="emit('select', contact)">
              <span class="truncate text-sm">{{ contact.name || t('lista.sinNombre') }}</span>
              <span v-if="contact.organization" class="truncate text-tx-muted text-xs">
                {{ contact.organization }}
              </span>
            </button>
          </li>
        </ul>
      </section>
    </template>
  </div>
</template>
