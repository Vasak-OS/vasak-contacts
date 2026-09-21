<script lang="ts" setup>
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { AlertMessage } from '@vasakgroup/vue-libvasak';
import type { Cuenta } from '@/composables/use-agenda';

defineProps<{ cuentas: Cuenta[]; avisos: string[] }>();

const { t } = useI18n();
</script>

<template>
  <aside class="flex w-52 shrink-0 flex-col gap-3 overflow-y-auto rounded-corner border border-ui-border bg-ui-surface/45 p-3">
    <!-- Sin ninguna cuenta, lo que hace falta es decir **qué hacer**. Una lista
         vacía sin explicación se lee como una aplicación rota. -->
    <div v-if="cuentas.length === 0" class="flex flex-col gap-1">
      <p class="font-medium text-sm">{{ t('cuentas.sinCuentasTitulo') }}</p>
      <p class="text-tx-muted text-xs">{{ t('cuentas.sinCuentasDescripcion') }}</p>
    </div>

    <section v-else class="flex flex-col gap-2">
      <h2 class="font-medium text-tx-muted text-xs uppercase">{{ t('cuentas.titulo') }}</h2>
      <ul class="flex flex-col gap-1">
        <li v-for="cuenta in cuentas" :key="cuenta.id" class="flex flex-col">
          <span class="truncate text-sm" :title="cuenta.nombre">{{ cuenta.nombre }}</span>
          <!-- Una cuenta que hay que reconectar se muestra igual, con el aviso
               al lado: sacarla de la lista se ve como una cuenta borrada, y la
               persona no se enteraría de que le falta hacer algo. -->
          <span v-if="cuenta.necesita_reconectarse" class="text-status-warning text-xs">
            {{ t('cuentas.necesitaReconectarse') }}
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
      v-if="avisos.length > 0"
      tone="warning"
      icon="dialog-warning"
      :title="t('cuentas.noSePudoLeerTodo')">
      <ul class="flex flex-col gap-1 text-xs">
        <li v-for="aviso in avisos" :key="aviso">{{ aviso }}</li>
      </ul>
    </AlertMessage>
  </aside>
</template>
