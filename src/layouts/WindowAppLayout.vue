<script lang="ts" setup>
/**
 * La ventana de la agenda.
 *
 * No dibuja nada propio: el borde, la esquina, el fondo, la barra y los tres
 * botones salen de `WindowFrame`, que es el mismo de todas las ventanas del
 * escritorio. Estaba copiado acá, y ya había derivado de las copias vecinas.
 *
 * De arriba viene además algo que esta copia no tenía: la barra puede ir
 * arriba, abajo, a la izquierda o a la derecha según `window.barPosition` en
 * `~/.config/vasak/vasak.conf`.
 *
 * Las ranuras van tal cual al marco. `barra` es el contenido, que es lo único
 * que crece y donde va el buscador: centrado ahí queda en el medio de lo que
 * sobra entre el icono y los controles, que es donde el ojo lo busca. `centro`
 * sigue significando lo que dice —centrado respecto de la ventana entera— y
 * queda para lo que de verdad lo necesite; acá ya no lo usa nadie.
 */
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { WindowFrame } from '@vasakgroup/vue-libvasak';

const { t } = useI18n();
</script>

<template>
  <WindowFrame
    :minimize-label="t('ventana.minimizar')"
    :maximize-label="t('ventana.maximizar')"
    :close-label="t('ventana.cerrar')">
    <template v-if="$slots.identidad" #identidad><slot name="identidad" /></template>
    <template v-if="$slots.barra" #barra><slot name="barra" /></template>
    <template v-if="$slots.barraCentro" #centro><slot name="barraCentro" /></template>
    <template v-if="$slots.acciones" #acciones><slot name="acciones" /></template>

    <div class="flex min-h-0 min-w-0 flex-1 gap-1 p-1">
      <slot />
    </div>
  </WindowFrame>
</template>
