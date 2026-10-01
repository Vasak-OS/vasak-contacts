<script lang="ts" setup>
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import {
	ActionButton,
	AlertMessage,
	EmptyState,
	Panel,
	SectionHeading,
	StatusDot,
} from '@vasakgroup/vue-libvasak';
import type { Cuenta } from '@/composables/use-agenda';
import { NARROW_ONLY, NARROW_WRAP } from '@/tools/narrow-layout';

defineProps<{ accounts: Cuenta[]; notices: string[] }>();
const emit = defineEmits<{
	/** Ir a la lista, con la ventana angosta. */
	forward: [];
}>();

const { t } = useI18n();
</script>

<template>
  <Panel as="aside" padding="none" scroll class="w-52 max-w-[25%] shrink-0 gap-3 p-3">
    <!-- El ancho es el de siempre (`w-52`) mientras haya lugar, pero nunca más
         de un cuarto de la fila: con el ancho fijo solo, en una ventana angosta
         las dos columnas de la izquierda se quedaban con todo y la ficha
         desaparecía. Desde unos 830 px de fila mide lo mismo que antes. El
         comentario va adentro: arriba de la raíz la volvería un fragmento. -->

    <!-- Con la ventana angosta, una columna por vez: las cuentas son la
         primera, y de acá se sigue a la lista. Con la ventana ancha no existe. -->
    <div class="flex justify-end" :class="NARROW_ONLY">
      <ActionButton
        variant="ghost"
        size="sm"
        icon="go-next"
        icon-type="symbol"
        icon-right
        :label="t('nav.contacts')"
        v-bind="{ 'data-nav': '' }"
        @click="emit('forward')" />
    </div>

    <!-- Sin ninguna cuenta, lo que hace falta es decir **qué hacer**. Una lista
         vacía sin explicación se lee como una aplicación rota. -->
    <EmptyState
      v-if="accounts.length === 0"
      size="sm"
      icon=""
      :title="t('cuentas.sinCuentasTitulo')"
      :note="t('cuentas.sinCuentasDescripcion')" />

    <section v-else class="flex flex-col gap-2">
      <SectionHeading as="h2" :title="t('cuentas.titulo')" />
      <ul class="flex flex-col gap-1">
        <li v-for="account in accounts" :key="account.id" class="flex min-w-0 flex-col">
          <span class="truncate text-sm" :class="NARROW_WRAP" :title="account.nombre">{{ account.nombre }}</span>
          <!-- Una cuenta que hay que reconectar se muestra igual, con el aviso
               al lado: sacarla de la lista se ve como una cuenta borrada, y la
               persona no se enteraría de que le falta hacer algo.

               El punto del tono y el texto atenuado, y no el texto en amarillo:
               el color de aviso del esquema no llega a 4,5:1 como color de
               texto sobre el panel en claro. -->
          <span
            v-if="account.necesita_reconectarse"
            class="flex items-start gap-1.5 text-tx-muted text-xs"
            data-testid="needs-reconnect">
            <StatusDot tone="warning" class="mt-1" />
            <span class="min-w-0 break-words">{{ t('cuentas.necesitaReconectarse') }}</span>
          </span>
        </li>
      </ul>
    </section>

    <!-- Lo que no se pudo leer va a la vista y no a la consola: una libreta
         vacía y una que falló se ven idénticas, y «no tengo a nadie anotado» y
         «no sé a quién tengo anotado» no son lo mismo.

         En el aviso del sistema: el amarillo, el borde y el rol salían de una
         copia a mano de la misma tabla. Es `warning` y no `error` a propósito
         —la agenda funciona, sólo que incompleta—, y con eso el rol sigue
         siendo `status`: espera turno en vez de interrumpir. -->
    <AlertMessage
      v-if="notices.length > 0"
      tone="warning"
      icon="dialog-warning"
      :title="t('cuentas.noSePudoLeerTodo')">
      <ul class="flex flex-col gap-1 text-xs">
        <li v-for="notice in notices" :key="notice" class="break-words">{{ notice }}</li>
      </ul>
    </AlertMessage>
  </Panel>
</template>
