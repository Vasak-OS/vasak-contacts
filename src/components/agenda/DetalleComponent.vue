<script lang="ts" setup>
import { open as abrirConElSistema } from '@tauri-apps/plugin-shell';
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { ref } from 'vue';
import type { Contacto, Dato } from '@/tools/agenda';
import { enlaceDeCorreo, enlaceDeTelefono } from '@/tools/enlaces';

defineProps<{ contacto: Contacto | null }>();

const { t } = useI18n();

/** Lo último que se copió, para decirlo un momento. */
const copiado = ref('');

async function copiar(valor: string) {
	try {
		await navigator.clipboard.writeText(valor);
		copiado.value = valor;
		setTimeout(() => {
			if (copiado.value === valor) {
				copiado.value = '';
			}
		}, 2000);
	} catch (e) {
		// Que no se pueda copiar no rompe nada: el valor está a la vista y se
		// puede seleccionar. Queda en la consola por si pasa siempre.
		console.error('no se pudo copiar', e);
	}
}

/**
 * Abre el correo o el teléfono con la aplicación que corresponda.
 *
 * Por el `shell` del sistema y no armando la ventana nosotros: `mailto:` lo
 * abre la aplicación de correo que la persona eligió, que puede no ser la
 * nuestra — y eso es lo correcto.
 *
 * Cómo se arma cada enlace está en `enlaces.ts`, con sus dos trampas: la de
 * correo hay que codificarla y la de teléfono hay que limpiarla.
 */
async function abrir(enlace: string) {
	if (!enlace) {
		return;
	}
	try {
		await abrirConElSistema(enlace);
	} catch (e) {
		console.error('no se pudo abrir', e);
	}
}

/** La etiqueta que puso quien hizo la tarjeta, o nada. */
function etiqueta(dato: Dato): string {
	return dato.tipo ? `${dato.tipo} · ` : '';
}
</script>

<template>
  <section class="flex min-w-0 flex-1 flex-col overflow-y-auto rounded-corner border border-ui-border bg-ui-surface/45">
    <p v-if="!contacto" class="p-4 text-tx-muted text-sm">{{ t('contacto.elegiUno') }}</p>

    <template v-else>
      <header class="flex flex-col gap-1 border-ui-border border-b p-4">
        <h1 class="font-title text-xl">{{ contacto.nombre || t('lista.sinNombre') }}</h1>
        <p v-if="contacto.organizacion" class="text-tx-muted text-sm">
          {{ contacto.organizacion }}
        </p>
      </header>

      <div class="flex flex-col gap-4 p-4">
        <!-- La clave lleva la posición y la etiqueta, no sólo el valor: un
             contacto puede tener el mismo número anotado como «casa» y como
             «celular», y con claves repetidas Vue reusa la fila equivocada al
             actualizar. -->
        <section v-if="contacto.correos.length > 0" class="flex flex-col gap-1">
          <h2 class="font-medium text-tx-muted text-xs uppercase">{{ t('contacto.correos') }}</h2>
          <div
            v-for="(correo, i) in contacto.correos"
            :key="`${i}-${correo.tipo}-${correo.valor}`"
            class="flex items-center gap-2">
            <span class="min-w-0 flex-1 truncate text-sm">
              <span class="text-tx-muted text-xs">{{ etiqueta(correo) }}</span>{{ correo.valor }}
            </span>
            <button
              type="button"
              class="rounded-corner px-2 py-0.5 text-sm hover:bg-ui-surface"
              @click="abrir(enlaceDeCorreo(correo.valor))">
              {{ t('contacto.escribir') }}
            </button>
            <button
              type="button"
              class="rounded-corner px-2 py-0.5 text-tx-muted text-sm hover:bg-ui-surface"
              @click="copiar(correo.valor)">
              {{ copiado === correo.valor ? t('contacto.copiado') : t('contacto.copiar') }}
            </button>
          </div>
        </section>

        <section v-if="contacto.telefonos.length > 0" class="flex flex-col gap-1">
          <h2 class="font-medium text-tx-muted text-xs uppercase">
            {{ t('contacto.telefonos') }}
          </h2>
          <div
            v-for="(telefono, i) in contacto.telefonos"
            :key="`${i}-${telefono.tipo}-${telefono.valor}`"
            class="flex items-center gap-2">
            <span class="min-w-0 flex-1 truncate text-sm">
              <span class="text-tx-muted text-xs">{{ etiqueta(telefono) }}</span
              >{{ telefono.valor }}
            </span>
            <!-- Sin botón si el «número» no tiene dígitos: uno que no hace
                 nada al apretarlo es peor que no estar. -->
            <button
              v-if="enlaceDeTelefono(telefono.valor)"
              type="button"
              class="rounded-corner px-2 py-0.5 text-sm hover:bg-ui-surface"
              @click="abrir(enlaceDeTelefono(telefono.valor))">
              {{ t('contacto.llamar') }}
            </button>
            <button
              type="button"
              class="rounded-corner px-2 py-0.5 text-tx-muted text-sm hover:bg-ui-surface"
              @click="copiar(telefono.valor)">
              {{ copiado === telefono.valor ? t('contacto.copiado') : t('contacto.copiar') }}
            </button>
          </div>
        </section>

        <section v-if="contacto.notas.trim()" class="flex flex-col gap-1">
          <h2 class="font-medium text-tx-muted text-xs uppercase">{{ t('contacto.notas') }}</h2>
          <!-- `pre-wrap` y no HTML: la nota la escribió quien hizo la tarjeta,
               que puede ser cualquiera. Se muestra, no se interpreta. -->
          <pre class="whitespace-pre-wrap break-words font-sans text-sm">{{ contacto.notas }}</pre>
        </section>

        <p class="text-tx-muted text-xs">{{ t('contacto.soloLectura') }}</p>
      </div>
    </template>
  </section>
</template>
