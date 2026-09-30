<script lang="ts" setup>
import { open as openWithSystem } from '@tauri-apps/plugin-shell';
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { computed, ref } from 'vue';
import ContactPhotoComponent from '@/components/agenda/ContactPhotoComponent.vue';
import type { Contact } from '@/tools/address-book';
import {
	addressLines,
	customFieldName,
	formatPartialDate,
	labelKey,
	languageName,
	serviceName,
	webLink,
} from '@/tools/contact-fields';
import { enlaceDeCorreo, enlaceDeTelefono } from '@/tools/enlaces';

const props = defineProps<{ contact: Contact | null }>();

const { t, locale } = useI18n();

/** Lo último que se copió, para decirlo un momento. */
const copied = ref('');

async function copy(value: string) {
	try {
		await navigator.clipboard.writeText(value);
		copied.value = value;
		setTimeout(() => {
			if (copied.value === value) {
				copied.value = '';
			}
		}, 2000);
	} catch (e) {
		// Que no se pueda copiar no rompe nada: el valor está a la vista y se
		// puede seleccionar. Queda en la consola por si pasa siempre.
		console.error('no se pudo copiar', e);
	}
}

/**
 * Abre el correo, el teléfono o la web con la aplicación que corresponda.
 *
 * Por el `shell` del sistema (`xdg-open`) y no armando la ventana nosotros:
 * `mailto:` lo abre la aplicación de correo que la persona eligió, y una web
 * el navegador que eligió — que puede no ser el nuestro, y eso es lo correcto.
 *
 * Cómo se arma cada enlace está en `enlaces.ts` y en `contact-fields.ts`, con
 * sus trampas: la de correo hay que codificarla, la de teléfono limpiarla, y
 * una web sólo se abre si es `http` o `https`.
 */
async function openLink(link: string) {
	if (!link) {
		return;
	}
	try {
		await openWithSystem(link);
	} catch (e) {
		console.error('no se pudo abrir', e);
	}
}

/**
 * La etiqueta que puso quien hizo la tarjeta, traducida si es una de las de
 * siempre; o nada.
 */
function labelText(label: string): string {
	if (!label) {
		return '';
	}
	const key = labelKey(label);
	return key ? t(key) : label;
}

/** La etiqueta con el separador, para ir delante del valor. */
function labelPrefix(label: string): string {
	const text = labelText(label);
	return text ? `${text} · ` : '';
}

/** El cargo y la función juntos, sin repetir si dicen lo mismo. */
const position = computed(() => {
	const c = props.contact;
	if (!c) return '';
	const parts = [c.title, c.role].map((p) => p.trim()).filter((p) => p.length > 0);
	return [...new Set(parts)].join(' · ');
});

/** Las fechas que hay, con su nombre. */
const dates = computed(() => {
	const c = props.contact;
	if (!c) return [];
	const out: { key: string; label: string; text: string }[] = [];
	if (c.birthday) {
		out.push({
			key: 'birthday',
			label: t('contact.birthday'),
			text: formatPartialDate(c.birthday, locale.value),
		});
	}
	if (c.anniversary) {
		out.push({
			key: 'anniversary',
			label: t('contact.anniversary'),
			text: formatPartialDate(c.anniversary, locale.value),
		});
	}
	c.other_dates.forEach((d, i) => {
		const key = labelKey(d.label);
		out.push({
			key: `other-${i}`,
			label: key ? t(key) : d.label || t('contact.otherDate'),
			text: formatPartialDate(d.date, locale.value),
		});
	});
	return out.filter((d) => d.text);
});

/** Si hay algo para «Más datos»: lo que se consulta poco va plegado. */
const hasMore = computed(() => {
	const c = props.contact;
	return !!c && (c.languages.length > 0 || !!c.time_zone || !!c.geo || c.custom_fields.length > 0);
});

/** Por qué no se ve la foto, cuando vale la pena decirlo. */
const photoNote = computed(() => {
	switch (props.contact?.photo_skipped) {
		case 'too_large':
			return t('contact.photoTooLarge');
		case 'insecure':
			return t('contact.photoInsecure');
		default:
			return '';
	}
});

/** Qué es la tarjeta, cuando no es una persona. */
const kindLabel = computed(() => {
	const kind = props.contact?.kind;
	return kind === 'org' || kind === 'group' || kind === 'location'
		? t(`contact.kinds.${kind}`)
		: '';
});
</script>

<template>
  <section class="flex min-w-0 flex-1 flex-col overflow-y-auto rounded-corner border border-ui-border bg-ui-surface/45">
    <p v-if="!contact" class="p-4 text-tx-muted text-sm">{{ t('contact.pickOne') }}</p>

    <template v-else>
      <header class="flex items-center gap-4 border-ui-border border-b p-4">
        <ContactPhotoComponent :contact="contact" />
        <div class="flex min-w-0 flex-col gap-1">
          <h1 class="font-title text-xl">{{ contact.name || t('lista.sinNombre') }}</h1>
          <p v-if="contact.nickname" class="text-tx-muted text-sm" data-testid="nickname">
            «{{ contact.nickname }}»
          </p>
          <p v-if="position || contact.organization" class="text-sm" data-testid="position">
            <span v-if="position">{{ position }}</span>
            <span v-if="position && contact.organization" class="text-tx-muted"> — </span>
            <span v-if="contact.organization" class="text-tx-muted">{{ contact.organization }}</span>
          </p>
          <p v-if="kindLabel" class="text-tx-muted text-xs">{{ kindLabel }}</p>
          <ul v-if="contact.categories.length > 0" class="flex flex-wrap gap-1" :aria-label="t('contact.categories')">
            <li
              v-for="category in contact.categories"
              :key="category"
              class="rounded-corner-sm border border-ui-border px-1.5 text-tx-muted text-xs">
              {{ category }}
            </li>
          </ul>
        </div>
      </header>

      <div class="flex flex-col gap-4 p-4">
        <p v-if="photoNote" class="text-tx-muted text-xs" data-testid="photo-note">{{ photoNote }}</p>

        <!-- La clave lleva la posición y la etiqueta, no sólo el valor: un
             contacto puede tener el mismo número anotado como «casa» y como
             «celular», y con claves repetidas Vue reusa la fila equivocada al
             actualizar. -->
        <section v-if="contact.emails.length > 0" class="flex flex-col gap-1">
          <h2 class="font-medium text-tx-muted text-xs uppercase">{{ t('contact.emails') }}</h2>
          <div
            v-for="(email, i) in contact.emails"
            :key="`${i}-${email.label}-${email.value}`"
            class="flex items-center gap-2">
            <span class="min-w-0 flex-1 truncate text-sm">
              <span class="text-tx-muted text-xs">{{ labelPrefix(email.label) }}</span>{{ email.value }}
            </span>
            <button
              type="button"
              class="rounded-corner px-2 py-0.5 text-sm hover:bg-ui-surface"
              @click="openLink(enlaceDeCorreo(email.value))">
              {{ t('contact.write') }}
            </button>
            <button
              type="button"
              class="rounded-corner px-2 py-0.5 text-tx-muted text-sm hover:bg-ui-surface"
              @click="copy(email.value)">
              {{ copied === email.value ? t('contact.copied') : t('contact.copy') }}
            </button>
          </div>
        </section>

        <section v-if="contact.phones.length > 0" class="flex flex-col gap-1">
          <h2 class="font-medium text-tx-muted text-xs uppercase">{{ t('contact.phones') }}</h2>
          <div
            v-for="(phone, i) in contact.phones"
            :key="`${i}-${phone.label}-${phone.value}`"
            class="flex items-center gap-2">
            <span class="min-w-0 flex-1 truncate text-sm">
              <span class="text-tx-muted text-xs">{{ labelPrefix(phone.label) }}</span>{{ phone.value }}
            </span>
            <!-- Sin botón si el «número» no tiene dígitos: uno que no hace
                 nada al apretarlo es peor que no estar. -->
            <button
              v-if="enlaceDeTelefono(phone.value)"
              type="button"
              class="rounded-corner px-2 py-0.5 text-sm hover:bg-ui-surface"
              @click="openLink(enlaceDeTelefono(phone.value))">
              {{ t('contact.call') }}
            </button>
            <button
              type="button"
              class="rounded-corner px-2 py-0.5 text-tx-muted text-sm hover:bg-ui-surface"
              @click="copy(phone.value)">
              {{ copied === phone.value ? t('contact.copied') : t('contact.copy') }}
            </button>
          </div>
        </section>

        <!-- La dirección en renglones, en un `address` y no en un párrafo: es
             lo que es, y un lector de pantalla lo dice. -->
        <section v-if="contact.addresses.length > 0" class="flex flex-col gap-2" data-testid="addresses">
          <h2 class="font-medium text-tx-muted text-xs uppercase">{{ t('contact.addresses') }}</h2>
          <div
            v-for="(address, i) in contact.addresses"
            :key="`${i}-${address.label}-${address.street}`"
            class="flex items-start gap-2">
            <div class="min-w-0 flex-1 text-sm">
              <span v-if="address.label" class="text-tx-muted text-xs">{{ labelText(address.label) }}</span>
              <address class="not-italic">
                <span v-for="(line, j) in addressLines(address)" :key="j" class="block">{{ line }}</span>
              </address>
            </div>
            <button
              type="button"
              class="rounded-corner px-2 py-0.5 text-tx-muted text-sm hover:bg-ui-surface"
              @click="copy(addressLines(address).join('\n'))">
              {{ copied === addressLines(address).join('\n') ? t('contact.copied') : t('contact.copy') }}
            </button>
          </div>
        </section>

        <section v-if="dates.length > 0" class="flex flex-col gap-1" data-testid="dates">
          <h2 class="font-medium text-tx-muted text-xs uppercase">{{ t('contact.dates') }}</h2>
          <p v-for="date in dates" :key="date.key" class="text-sm">
            <span class="text-tx-muted text-xs">{{ date.label }} · </span>{{ date.text }}
          </p>
        </section>

        <section v-if="contact.websites.length > 0" class="flex flex-col gap-1" data-testid="websites">
          <h2 class="font-medium text-tx-muted text-xs uppercase">{{ t('contact.websites') }}</h2>
          <div
            v-for="(site, i) in contact.websites"
            :key="`${i}-${site.value}`"
            class="flex items-center gap-2">
            <span class="min-w-0 flex-1 truncate text-sm">
              <span class="text-tx-muted text-xs">{{ labelPrefix(site.label) }}</span>{{ site.value }}
            </span>
            <!-- Sólo si es una web de verdad: una `javascript:` no se abre. -->
            <button
              v-if="webLink(site.value)"
              type="button"
              class="rounded-corner px-2 py-0.5 text-sm hover:bg-ui-surface"
              @click="openLink(webLink(site.value))">
              {{ t('contact.open') }}
            </button>
            <button
              type="button"
              class="rounded-corner px-2 py-0.5 text-tx-muted text-sm hover:bg-ui-surface"
              @click="copy(site.value)">
              {{ copied === site.value ? t('contact.copied') : t('contact.copy') }}
            </button>
          </div>
        </section>

        <section v-if="contact.social.length > 0" class="flex flex-col gap-1" data-testid="social">
          <h2 class="font-medium text-tx-muted text-xs uppercase">{{ t('contact.social') }}</h2>
          <div
            v-for="(profile, i) in contact.social"
            :key="`${i}-${profile.service}-${profile.handle}`"
            class="flex items-center gap-2">
            <span class="min-w-0 flex-1 truncate text-sm">
              <span class="text-tx-muted text-xs">{{
                serviceName(profile.service) ? `${serviceName(profile.service)} · ` : labelPrefix(profile.label)
              }}</span>{{ profile.handle }}
            </span>
            <button
              v-if="webLink(profile.url)"
              type="button"
              class="rounded-corner px-2 py-0.5 text-sm hover:bg-ui-surface"
              @click="openLink(webLink(profile.url))">
              {{ t('contact.open') }}
            </button>
            <button
              type="button"
              class="rounded-corner px-2 py-0.5 text-tx-muted text-sm hover:bg-ui-surface"
              @click="copy(profile.handle)">
              {{ copied === profile.handle ? t('contact.copied') : t('contact.copy') }}
            </button>
          </div>
        </section>

        <section v-if="contact.notes.trim()" class="flex flex-col gap-1">
          <h2 class="font-medium text-tx-muted text-xs uppercase">{{ t('contact.notes') }}</h2>
          <!-- `pre-wrap` y no HTML: la nota la escribió quien hizo la tarjeta,
               que puede ser cualquiera. Se muestra, no se interpreta. -->
          <pre class="whitespace-pre-wrap break-words font-sans text-sm">{{ contact.notes }}</pre>
        </section>

        <!-- Lo que se consulta poco, plegado: sin esto la ficha de alguien con
             todo cargado es un formulario de treinta renglones. -->
        <details v-if="hasMore" class="flex flex-col gap-1" data-testid="more">
          <summary class="cursor-pointer font-medium text-tx-muted text-xs uppercase">
            {{ t('contact.more') }}
          </summary>
          <dl class="mt-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
            <template v-if="contact.languages.length > 0">
              <dt class="text-tx-muted">{{ t('contact.languages') }}</dt>
              <dd>{{ contact.languages.map((tag) => languageName(tag, locale)).join(', ') }}</dd>
            </template>
            <template v-if="contact.time_zone">
              <dt class="text-tx-muted">{{ t('contact.timeZone') }}</dt>
              <dd>{{ contact.time_zone }}</dd>
            </template>
            <template v-if="contact.geo">
              <dt class="text-tx-muted">{{ t('contact.location') }}</dt>
              <dd class="tabular-nums">{{ contact.geo }}</dd>
            </template>
            <template v-for="(field, i) in contact.custom_fields" :key="`${i}-${field.name}`">
              <dt class="text-tx-muted">{{ customFieldName(field) }}</dt>
              <dd class="break-words">{{ field.value }}</dd>
            </template>
          </dl>
        </details>

        <p class="text-tx-muted text-xs">{{ t('contact.readOnly') }}</p>
      </div>
    </template>
  </section>
</template>
