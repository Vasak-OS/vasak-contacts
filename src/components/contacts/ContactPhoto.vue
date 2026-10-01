<script lang="ts" setup>
import { invoke } from '@tauri-apps/api/core';
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { Avatar } from '@vasakgroup/vue-libvasak';
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
/**
 * Si el motor no pudo dibujar la foto. El `Avatar` ya vuelve solo a las
 * iniciales; esto es para que entonces no se anuncien como «Foto de…».
 */
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

/**
 * El nombre que se le pasa al `Avatar` para que dibuje **nuestras** iniciales.
 *
 * Las de la librería son las de las dos primeras palabras («Ana María Pérez» →
 * «AM») y toman cualquier carácter («+54 11…» → «+1»); las de los contactos son
 * la primera y la última, y sólo de palabras que empiezan con una letra. Para
 * que la cabecera diga lo mismo que antes, se le pasan nuestras iniciales
 * separadas, y la librería las junta tal cual. Sin iniciales, el icono genérico
 * del tema en lugar del círculo vacío.
 */
const initialsName = computed(() => Array.from(initialsOf(props.contact.name)).join(' '));
/**
 * El nombre de la foto, sólo cuando hay foto: las iniciales no son una «Foto
 * de…», y el nombre ya está escrito al lado.
 */
const alt = computed(() =>
	source.value && !broken.value
		? interpolar(t('contact.photoAlt'), props.contact.name || t('lista.sinNombre'))
		: ''
);
</script>

<script lang="ts">
/**
 * Uno solo para toda la ventana: la foto de alguien se le pide al programa una
 * vez por sesión, abra uno su ficha las veces que la abra.
 */
const loadPhoto = createPhotoLoader(invoke);

// El `Avatar` va en `xl`, la caja de 64 px de siempre: redonda y del mismo
// tamaño con foto y sin ella, para que la cabecera no salte cuando la foto
// llega. La nota va acá y no en la plantilla: un comentario arriba de la raíz
// la volvería un fragmento.
</script>

<template>
  <Avatar
    :src="source || null"
    :name="initialsName"
    :alt="alt"
    size="xl"
    v-bind="{ 'data-testid': 'contact-photo' }"
    @error="broken = true" />
</template>
