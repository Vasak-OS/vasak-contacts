<script lang="ts" setup>
import { open as openWithSystem } from '@tauri-apps/plugin-shell';
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import {
	ActionButton,
	Badge,
	Disclosure,
	Panel,
	type PropertyItem,
	PropertyList,
	SectionHeading,
} from '@vasakgroup/vue-libvasak';
import { computed, ref } from 'vue';
import ContactPhoto from '@/components/contacts/ContactPhoto.vue';
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
import { NARROW_FULL_LINE, NARROW_ONLY, NARROW_ROW_WRAP, NARROW_WRAP } from '@/tools/narrow-layout';

const props = defineProps<{ contact: Contact | null }>();
const emit = defineEmits<{
	/** Volver a la lista, con la ventana angosta. */
	back: [];
}>();

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

/**
 * Lo de «Más datos», como pares de nombre y valor para la `PropertyList`: lo
 * que se consulta poco va plegado. El `id` es para el `key`: dos campos propios
 * pueden llamarse igual.
 */
const moreItems = computed<PropertyItem[]>(() => {
	const c = props.contact;
	if (!c) return [];
	const out: PropertyItem[] = [];
	if (c.languages.length > 0) {
		out.push({
			id: 'languages',
			label: t('contact.languages'),
			value: c.languages.map((tag) => languageName(tag, locale.value)).join(', '),
		});
	}
	if (c.time_zone) {
		out.push({ id: 'time-zone', label: t('contact.timeZone'), value: c.time_zone });
	}
	if (c.geo) {
		out.push({ id: 'geo', label: t('contact.location'), value: c.geo });
	}
	c.custom_fields.forEach((field, i) => {
		out.push({
			id: `custom-${i}-${field.name}`,
			label: customFieldName(field),
			value: field.value,
		});
	});
	return out;
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
  <Panel as="section" padding="none" scroll class="@container flex-1">
    <!-- Contenedor de consultas: en una ventana angosta la ficha puede quedar
         en unos 130 px, y ahí la cabecera pone la foto arriba del nombre y cada
         fila pasa los botones abajo del valor, en vez de partir las palabras
         letra por letra. Desde 12rem de ficha —la ventana de 600 px ya los
         tiene— se ve igual que siempre. El comentario va adentro: arriba de la
         raíz la volvería un fragmento. -->

    <!-- Con la ventana angosta, una columna por vez: de la ficha se vuelve a la
         lista, que conserva el contacto elegido. Con la ventana ancha no
         existe, y la ficha mide lo mismo que siempre. -->
    <div class="flex border-ui-line-weak border-b p-1" :class="NARROW_ONLY">
      <ActionButton
        variant="ghost"
        size="sm"
        icon="go-previous"
        icon-type="symbol"
        :label="t('nav.contacts')"
        v-bind="{ 'data-nav': '' }"
        @click="emit('back')" />
    </div>

    <p v-if="!contact" class="p-4 text-tx-muted text-sm">{{ t('contact.pickOne') }}</p>

    <template v-else>
      <header
        class="flex flex-col items-start gap-4 border-ui-line-weak border-b p-4 @min-[12rem]:flex-row @min-[12rem]:items-center">
        <ContactPhoto :contact="contact" />
        <div class="flex min-w-0 flex-col gap-1">
          <h1 class="break-words font-title text-xl">{{ contact.name || t('lista.sinNombre') }}</h1>
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
            <!-- Una insignia de contorno: es una etiqueta de quien hizo la tarjeta,
                 no un estado, y no tiene que competir con el nombre. -->
            <li v-for="category in contact.categories" :key="category" class="flex min-w-0">
              <Badge variant="outline" :label="category" />
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
          <SectionHeading as="h2" :title="t('contact.emails')" />
          <div
            v-for="(email, i) in contact.emails"
            :key="`${i}-${email.label}-${email.value}`"
            class="flex items-center gap-2 @max-[12rem]:flex-wrap"
            :class="NARROW_ROW_WRAP">
            <span class="min-w-0 flex-1 truncate text-sm @max-[12rem]:basis-full" :class="[NARROW_WRAP, NARROW_FULL_LINE]">
              <span class="text-tx-muted text-xs">{{ labelPrefix(email.label) }}</span>{{ email.value }}
            </span>
            <ActionButton
              class="shrink-0"
              variant="ghost"
              size="sm"
              :label="t('contact.write')"
              @click="openLink(enlaceDeCorreo(email.value))" />
            <ActionButton
              class="shrink-0"
              variant="ghost"
              size="sm"
              :label="copied === email.value ? t('contact.copied') : t('contact.copy')"
              @click="copy(email.value)" />
          </div>
        </section>

        <section v-if="contact.phones.length > 0" class="flex flex-col gap-1">
          <SectionHeading as="h2" :title="t('contact.phones')" />
          <div
            v-for="(phone, i) in contact.phones"
            :key="`${i}-${phone.label}-${phone.value}`"
            class="flex items-center gap-2 @max-[12rem]:flex-wrap"
            :class="NARROW_ROW_WRAP">
            <span class="min-w-0 flex-1 truncate text-sm @max-[12rem]:basis-full" :class="[NARROW_WRAP, NARROW_FULL_LINE]">
              <span class="text-tx-muted text-xs">{{ labelPrefix(phone.label) }}</span>{{ phone.value }}
            </span>
            <!-- Sin botón si el «número» no tiene dígitos: uno que no hace
                 nada al apretarlo es peor que no estar. -->
            <ActionButton
              class="shrink-0"
              v-if="enlaceDeTelefono(phone.value)"
              variant="ghost"
              size="sm"
              :label="t('contact.call')"
              @click="openLink(enlaceDeTelefono(phone.value))" />
            <ActionButton
              class="shrink-0"
              variant="ghost"
              size="sm"
              :label="copied === phone.value ? t('contact.copied') : t('contact.copy')"
              @click="copy(phone.value)" />
          </div>
        </section>

        <!-- La dirección en renglones, en un `address` y no en un párrafo: es
             lo que es, y un lector de pantalla lo dice. -->
        <section v-if="contact.addresses.length > 0" class="flex flex-col gap-2" data-testid="addresses">
          <SectionHeading as="h2" :title="t('contact.addresses')" />
          <div
            v-for="(address, i) in contact.addresses"
            :key="`${i}-${address.label}-${address.street}`"
            class="flex items-start gap-2 @max-[12rem]:flex-wrap"
            :class="NARROW_ROW_WRAP">
            <div class="min-w-0 flex-1 text-sm @max-[12rem]:basis-full" :class="NARROW_FULL_LINE">
              <span v-if="address.label" class="text-tx-muted text-xs">{{ labelText(address.label) }}</span>
              <address class="not-italic">
                <span v-for="(line, j) in addressLines(address)" :key="j" class="block">{{ line }}</span>
              </address>
            </div>
            <ActionButton
              class="shrink-0"
              variant="ghost"
              size="sm"
              :label="copied === addressLines(address).join('\n') ? t('contact.copied') : t('contact.copy')"
              @click="copy(addressLines(address).join('\n'))" />
          </div>
        </section>

        <section v-if="dates.length > 0" class="flex flex-col gap-1" data-testid="dates">
          <SectionHeading as="h2" :title="t('contact.dates')" />
          <p v-for="date in dates" :key="date.key" class="text-sm">
            <span class="text-tx-muted text-xs">{{ date.label }} · </span>{{ date.text }}
          </p>
        </section>

        <section v-if="contact.websites.length > 0" class="flex flex-col gap-1" data-testid="websites">
          <SectionHeading as="h2" :title="t('contact.websites')" />
          <div
            v-for="(site, i) in contact.websites"
            :key="`${i}-${site.value}`"
            class="flex items-center gap-2 @max-[12rem]:flex-wrap"
            :class="NARROW_ROW_WRAP">
            <span class="min-w-0 flex-1 truncate text-sm @max-[12rem]:basis-full" :class="[NARROW_WRAP, NARROW_FULL_LINE]">
              <span class="text-tx-muted text-xs">{{ labelPrefix(site.label) }}</span>{{ site.value }}
            </span>
            <!-- Sólo si es una web de verdad: una `javascript:` no se abre. -->
            <ActionButton
              class="shrink-0"
              v-if="webLink(site.value)"
              variant="ghost"
              size="sm"
              :label="t('contact.open')"
              @click="openLink(webLink(site.value))" />
            <ActionButton
              class="shrink-0"
              variant="ghost"
              size="sm"
              :label="copied === site.value ? t('contact.copied') : t('contact.copy')"
              @click="copy(site.value)" />
          </div>
        </section>

        <section v-if="contact.social.length > 0" class="flex flex-col gap-1" data-testid="social">
          <SectionHeading as="h2" :title="t('contact.social')" />
          <div
            v-for="(profile, i) in contact.social"
            :key="`${i}-${profile.service}-${profile.handle}`"
            class="flex items-center gap-2 @max-[12rem]:flex-wrap"
            :class="NARROW_ROW_WRAP">
            <span class="min-w-0 flex-1 truncate text-sm @max-[12rem]:basis-full" :class="[NARROW_WRAP, NARROW_FULL_LINE]">
              <span class="text-tx-muted text-xs">{{
                serviceName(profile.service) ? `${serviceName(profile.service)} · ` : labelPrefix(profile.label)
              }}</span>{{ profile.handle }}
            </span>
            <ActionButton
              class="shrink-0"
              v-if="webLink(profile.url)"
              variant="ghost"
              size="sm"
              :label="t('contact.open')"
              @click="openLink(webLink(profile.url))" />
            <ActionButton
              class="shrink-0"
              variant="ghost"
              size="sm"
              :label="copied === profile.handle ? t('contact.copied') : t('contact.copy')"
              @click="copy(profile.handle)" />
          </div>
        </section>

        <section v-if="contact.notes.trim()" class="flex flex-col gap-1">
          <SectionHeading as="h2" :title="t('contact.notes')" />
          <!-- `pre-wrap` y no HTML: la nota la escribió quien hizo la tarjeta,
               que puede ser cualquiera. Se muestra, no se interpreta. -->
          <pre class="whitespace-pre-wrap break-words font-sans text-sm">{{ contact.notes }}</pre>
        </section>

        <!-- Lo que se consulta poco, plegado: sin esto la ficha de alguien con
             cada campo cargado es un formulario de treinta renglones.

             Sin que la ficha cambie de largo (`tests/detail-layout.test.ts`):
             - cerrado, la cabecera del `Disclosure` es un botón de 32 px —el
               mínimo para tocarlo— y el `summary` de antes medía 20. Los
               márgenes negativos le devuelven esos 12 px (6 arriba, 6 abajo)
               y corren la flecha al borde, donde estaba el triángulo;
             - abierto, la región lleva 4 px más de relleno que el `mt-2` de
               antes y la `PropertyList` 4 px más entre filas que el `dl`: el
               margen de abajo crece a 14 px sólo entonces (`data-open`). -->
        <Disclosure
          v-if="moreItems.length > 0"
          :title="t('contact.more')"
          class="-mx-2 -my-1.5 data-[open=true]:-mb-3.5"
          v-bind="{ 'data-testid': 'more' }">
          <!-- Las coordenadas con cifras de ancho fijo, como antes, para que
               se lean en columna. -->
          <!-- Con la ventana ancha, nombre y valor en dos columnas como el `dl`
               de antes, aunque la ficha mida menos de 20rem (a 600 mide unos
               230 px y la lista ponía el nombre arriba del valor: 12 px más de
               ficha). Con la ventana angosta la ficha ocupa la fila entera y
               la lista se acomoda sola. -->
          <PropertyList
            :items="moreItems"
            class="@min-[36rem]/row:[&_dl]:grid-cols-[minmax(0,max-content)_minmax(0,1fr)] @min-[36rem]/row:[&_dl>div]:col-span-2">
            <template #value="{ item }">
              <span :class="item.id === 'geo' ? 'tabular-nums' : ''">{{ item.value }}</span>
            </template>
          </PropertyList>
        </Disclosure>

        <p class="text-tx-muted text-xs">{{ t('contact.readOnly') }}</p>
      </div>
    </template>
  </Panel>
</template>
