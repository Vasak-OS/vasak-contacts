<script lang="ts" setup>
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { computed } from 'vue';
import { type Contacto, inicialDe } from '@/tools/agenda';
import { interpolar } from '@/tools/interpolar';

const props = defineProps<{
	contactos: Contacto[];
	elegido: Contacto | null;
	consulta: string;
	cargando: boolean;
}>();
const emit = defineEmits<{ elegir: [contacto: Contacto] }>();

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
const grupos = computed(() => {
	const salida: { inicial: string; contactos: Contacto[] }[] = [];
	for (const contacto of props.contactos) {
		const inicial = inicialDe(contacto);
		const ultimo = salida.at(-1);
		if (ultimo?.inicial === inicial) {
			ultimo.contactos.push(contacto);
		} else {
			salida.push({ inicial, contactos: [contacto] });
		}
	}
	return salida;
});

/** El `#` no es una letra: se dice con palabras. */
function tituloDe(inicial: string): string {
	return inicial === '#' ? t('lista.otros') : inicial;
}
</script>

<template>
  <div class="flex w-72 shrink-0 flex-col overflow-y-auto border-ui-border border-r">
    <p v-if="cargando && contactos.length === 0" class="p-3 text-tx-muted text-sm" role="status">
      {{ t('lista.cargando') }}
    </p>

    <!-- Una búsqueda sin resultados dice **qué** no se encontró. «No hay nada»
         a secas deja a la persona sin saber si escribió mal o si de verdad no
         tiene a nadie anotado. -->
    <p v-else-if="contactos.length === 0 && consulta.trim()" class="p-3 text-tx-muted text-sm">
      {{ interpolar(t('lista.sinResultados'), consulta) }}
    </p>
    <p v-else-if="contactos.length === 0" class="p-3 text-tx-muted text-sm">
      {{ t('lista.vacia') }}
    </p>

    <template v-else>
      <section v-for="grupo in grupos" :key="grupo.inicial">
        <h3
          class="sticky top-0 bg-ui-bg/95 px-3 py-1 font-medium text-tx-muted text-xs uppercase">
          {{ tituloDe(grupo.inicial) }}
        </h3>
        <ul>
          <li v-for="contacto in grupo.contactos" :key="contacto.url || contacto.uid">
            <button
              type="button"
              class="flex w-full flex-col gap-0.5 px-3 py-1.5 text-left hover:bg-ui-surface/60"
              :class="{ 'bg-ui-surface': contacto.url === elegido?.url }"
              :aria-current="contacto.url === elegido?.url ? 'true' : undefined"
              @click="emit('elegir', contacto)">
              <span class="truncate text-sm">{{ contacto.nombre || t('lista.sinNombre') }}</span>
              <span v-if="contacto.organizacion" class="truncate text-tx-muted text-xs">
                {{ contacto.organizacion }}
              </span>
            </button>
          </li>
        </ul>
      </section>
    </template>
  </div>
</template>
