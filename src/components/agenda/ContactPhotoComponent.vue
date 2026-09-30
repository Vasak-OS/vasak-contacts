<script lang="ts" setup>
import { invoke } from '@tauri-apps/api/core';
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { computed, ref, watch } from 'vue';
import type { Contact } from '@/tools/address-book';
import { initialsOf } from '@/tools/contact-fields';
import { interpolar } from '@/tools/interpolar';
import { createPhotoLoader } from '@/tools/photo-loader';

const props = defineProps<{ contact: Contact }>();

const { t } = useI18n();

/**
 * La foto que se muestra: la de la tarjeta, o la que el programa bajó y
 * guardó. **Nunca** la dirección externa.
 */
const source = ref('');
/** Si el motor no pudo dibujar la foto: vuelven las iniciales. */
const broken = ref(false);

watch(
	() => [props.contact.photo, props.contact.photo_url] as const,
	async ([inline, remote]) => {
		broken.value = false;
		source.value = inline;
		if (inline || !remote) {
			return;
		}
		const loaded = await loadPhoto(remote);
		// Si mientras bajaba se eligió a otra persona, no se le pone la foto
		// de la anterior.
		if (props.contact.photo_url === remote && !props.contact.photo) {
			source.value = loaded;
		}
	},
	{ immediate: true }
);

const initials = computed(() => initialsOf(props.contact.name));
const alt = computed(() =>
	interpolar(t('contact.photoAlt'), props.contact.name || t('lista.sinNombre'))
);
</script>

<script lang="ts">
/**
 * Uno solo para toda la ventana: la foto de alguien se le pide al programa una
 * vez por sesión, abra uno su ficha las veces que la abra.
 */
const loadPhoto = createPhotoLoader(invoke);
</script>

<template>
  <div
    class="flex size-16 shrink-0 items-center justify-center overflow-hidden rounded-full bg-ui-surface font-title text-tx-muted text-xl"
    data-testid="contact-photo">
    <!-- Redonda y del mismo tamaño con foto y sin ella, para que la cabecera
         no salte cuando la foto llega. El comentario va adentro: arriba de la
         raíz la volvería un fragmento. -->
    <img
      v-if="source && !broken"
      :src="source"
      :alt="alt"
      class="size-full object-cover"
      @error="broken = true" />
    <span v-else aria-hidden="true">{{ initials }}</span>
  </div>
</template>
