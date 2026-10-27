<script lang="ts" setup>
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import {
	ActionButton,
	AlertMessage,
	BarSearch,
	EmptyState,
	RevealTransition,
	ThemeIcon,
} from '@vasakgroup/vue-libvasak';
import { computed, nextTick, onMounted, ref, useTemplateRef, watch } from 'vue';
import AccountsPanel from '@/components/contacts/AccountsPanel.vue';
import ConflictDialog from '@/components/contacts/ConflictDialog.vue';
import ContactDetail from '@/components/contacts/ContactDetail.vue';
import ContactForm from '@/components/contacts/ContactForm.vue';
import ContactList from '@/components/contacts/ContactList.vue';
import OwnCardDialog from '@/components/contacts/OwnCardDialog.vue';
import QrDialog from '@/components/contacts/QrDialog.vue';
import { useAgenda } from '@/composables/use-agenda';
import { useNarrowRow } from '@/composables/use-narrow-row';
import { useOwnCard } from '@/composables/use-own-card';
import WindowAppLayout from '@/layouts/WindowAppLayout.vue';
import type { Contact } from '@/tools/address-book';
import { claveSegunCantidad, interpolar } from '@/tools/interpolar';
import { type Pane, paneClass } from '@/tools/narrow-layout';
import { isStorePermissionDenied, reaskNoticeKey } from '@/tools/store-permission';

const { t, locale } = useI18n();
const {
	cuentas,
	contactos,
	visibles,
	elegido,
	consulta,
	cargando,
	avisos,
	sinPermiso,
	cargar,
	refresh,
	pedirPermisoDeNuevo,
	saveContact,
	deleteContact,
	getConflict,
	resolveConflict,
	elegir,
	syncStatuses,
	refreshFailures,
	addressBooks,
	hiddenBooks,
	categories,
	selectedCategories,
	toggleBook,
	toggleCategory,
	clearCategories,
} = useAgenda(() => locale.value);

/**
 * La tarjeta propia y el código QR (vasak-contacts#10). `qrContact` es de quién
 * es el código que se muestra: la ficha que se está mirando, o la tarjeta
 * propia con «mi QR».
 */
const own = useOwnCard(
	() => contactos.value,
	() => cuentas.value.map((cuenta) => cuenta.id)
);
const qrOpen = ref(false);
const qrContact = ref<Contact | null>(null);
const ownOpen = ref(false);
const ownLooking = ref(false);
const ownProposal = ref<Contact | null>(null);

function openQr(contact: Contact) {
	qrContact.value = contact;
	qrOpen.value = true;
}

/**
 * «Mi QR»: con la tarjeta propia marcada, su código. Sin marcar, se le pregunta
 * al servidor cuál es y la persona lo confirma; si no lo dice, el diálogo explica
 * que se marca desde la ficha («Este soy yo»).
 */
async function showMyQr() {
	if (own.ownCard.value) {
		openQr(own.ownCard.value);
		return;
	}
	ownProposal.value = null;
	ownLooking.value = true;
	ownOpen.value = true;
	try {
		ownProposal.value = await own.propose();
	} finally {
		ownLooking.value = false;
	}
}

function confirmOwnProposal() {
	const proposal = ownProposal.value;
	ownOpen.value = false;
	if (proposal) {
		own.mark(proposal);
		openQr(proposal);
	}
}

/**
 * El formulario de crear o editar. `editing` es el contacto que se edita, o
 * `null` para crear uno nuevo. `saving` desactiva los botones mientras el
 * almacén guarda, y `formError` muestra en el formulario por qué no se pudo.
 */
const formOpen = ref(false);
const editing = ref<Contact | null>(null);
const saving = ref(false);
const formError = ref('');

/** El error de borrar, cuando no se pudo; se muestra como un aviso y se cierra. */
const actionNotice = ref('');

/**
 * La comparación de un conflicto. `conflictContact` es el contacto que se está
 * resolviendo; el diálogo trae la comparación y resuelve con `useAgenda`.
 */
const conflictOpen = ref(false);
const conflictContact = ref<Contact | null>(null);

function openConflict(contact: Contact) {
	conflictContact.value = contact;
	conflictOpen.value = true;
}

/** Traduce el error de un comando de escritura para mostrarlo. */
function writeErrorText(error: string): string {
	return isStorePermissionDenied(error) ? t('form.denied') : t('form.error');
}

function openNew() {
	editing.value = null;
	formError.value = '';
	formOpen.value = true;
}

function openEdit(contact: Contact) {
	editing.value = contact;
	formError.value = '';
	formOpen.value = true;
}

/** Guarda lo que armó el formulario; si salió bien, lo cierra. */
async function submitForm(payload: {
	accountId: string;
	addressBookId: string;
	payload: Record<string, unknown>;
	ifMatch: string;
}) {
	if (saving.value) {
		return;
	}
	saving.value = true;
	formError.value = '';
	try {
		const error = await saveContact(payload);
		if (error) {
			formError.value = writeErrorText(error);
			return;
		}
		formOpen.value = false;
	} finally {
		saving.value = false;
	}
}

/** Borra un contacto ya confirmado; si no se pudo, lo dice. */
async function removeContact(contact: Contact) {
	const error = await deleteContact(contact);
	actionNotice.value = error ? writeErrorText(error) : '';
}

/**
 * El botón de «volver a pedir el permiso»: mientras pide se desactiva —no es
 * instantáneo y dos clics seguidos chocan con el techo de uno por minuto— y, si
 * no se pudo ni pedir, deja debajo el motivo traducido (el techo, o un recurso
 * ajeno). Si se pudo pedir, `pedirPermisoDeNuevo` ya releyó: o aparece la agenda,
 * o vuelve el mismo estado porque la persona dijo que no de nuevo.
 */
const pidiendoPermiso = ref(false);
const avisoPermiso = ref('');
async function reclamarPermiso() {
	if (pidiendoPermiso.value) {
		return;
	}
	avisoPermiso.value = '';
	pidiendoPermiso.value = true;
	try {
		const error = await pedirPermisoDeNuevo();
		avisoPermiso.value = error ? t(reaskNoticeKey(error)) : '';
	} finally {
		pidiendoPermiso.value = false;
	}
}

const countLabel = computed(() =>
	interpolar(t(claveSegunCantidad('lista.cuantos', visibles.value.length)), visibles.value.length)
);

/**
 * La columna que se mira con la ventana angosta (`tools/narrow-layout.ts`).
 * Se arranca por la lista, que es lo que se viene a buscar; las cuentas
 * quedan un paso atrás.
 */
const pane = ref<Pane>('list');

/**
 * Si la fila está angosta, para lo que no alcanza con las clases: la barra no
 * está adentro de la fila. Ahí el buscador se pliega en la lupa —un campo de 70
 * px se cortaba en «Bu»— y la cuenta de contactos baja a la lista, donde entra.
 */
const layout = useTemplateRef<InstanceType<typeof WindowAppLayout>>('layout');
const { narrow } = useNarrowRow(() => layout.value?.row);

/**
 * Pasa a otra columna y le lleva el foco a su botón de ir o volver: la que se
 * deja se oculta, y un foco en algo oculto se pierde. Con la ventana ancha ese
 * botón no se muestra y el foco se queda donde estaba.
 */
async function go(next: Pane) {
	pane.value = next;
	await nextTick();
	const target = document.querySelector<HTMLElement>(`[data-pane="${next}"] [data-nav]`);
	target?.focus();
}

/**
 * Buscar es mirar la lista: con la ventana angosta, escribir en el buscador
 * desde la ficha o desde las cuentas dejaba los resultados ocultos detrás de
 * la columna que se estaba viendo. Sin mover el foco, que está en el campo y
 * ahí se sigue escribiendo. Con la ventana ancha no se nota.
 */
watch(consulta, () => {
	pane.value = 'list';
});

/** Elegir a alguien es, además, ir a su ficha. */
function select(contact: Contact) {
	elegir(contact);
	void go('detail');
}

onMounted(cargar);
</script>

<template>
  <WindowAppLayout ref="layout">
    <!-- El icono de la aplicación, a la izquierda de todo, como en el resto
         del escritorio. Reemplaza al título escrito: el nombre de la ventana ya
         lo dice el icono, y el renglón que ocupaba era el que empujaba al
         buscador a un costado.

         Va en `identidad` y no en el contenido de la barra: es la única zona
         que no se desplaza con el resto cuando la barra queda a un costado. -->
    <template #identidad>
      <!-- A color y no monocromo: es la identidad de la ventana, como en el
           resto del escritorio.

           `contacts` y no `x-office-address-book`, que es el nombre más obvio:
           ese otro existe **también** como icono de tipo de archivo, y el tema
           resolvía el de `mimes/` — una hoja con una arroba, que no es la
           aplicación. Éste está sólo en `apps/`, así que no hay qué desempatar. -->
      <ThemeIcon name="contacts" :size="24" :alt="t('app.nombre')" />
    </template>

    <!-- El estado y el botón de actualizar, junto a los botones de la ventana,
         que es donde están en el resto de las aplicaciones. El hueco que los
         empujaba hasta ahí —un `span` con `flex-1`— lo pone la barra sola. -->
    <template #acciones>
      <!-- El estado de carga se dice, no se insinúa con un icono girando: sin
           esto, un servidor lento y una agenda vacía se ven igual. -->
      <span v-if="cargando" class="text-tx-muted text-xs" role="status">
        {{ t('lista.cargando') }}
      </span>
      <!-- Crear un contacto. Sólo si hay una libreta donde ponerlo —sin cuentas
           conectadas no hay dónde—, y nunca mientras el almacén está negado. -->
      <ActionButton
        v-if="!sinPermiso && addressBooks.length > 0"
        variant="ghost"
        label=""
        icon="contact-new"
        :icon-alt="t('form.newTitle')"
        :title="t('form.newTitle')"
        @click="openNew()" />
      <!-- Mi código QR: el de la tarjeta propia. Sólo si hay agenda que mirar. -->
      <ActionButton
        v-if="!sinPermiso && contactos.length > 0"
        variant="ghost"
        label=""
        icon="view-barcode-qr"
        :icon-alt="t('qr.mine')"
        :title="t('qr.mine')"
        v-bind="{ 'data-testid': 'my-qr' }"
        @click="showMyQr()" />
      <!-- `ghost`, como los tres botones de la ventana que tiene al lado: los
           cuatro son controles de la barra y se leen como un grupo. -->
      <ActionButton
        variant="ghost"
        label=""
        icon="view-refresh"
        :icon-alt="t('lista.actualizar')"
        :title="t('lista.actualizar')"
        :disabled="cargando"
        @click="refresh()" />
    </template>

    <!-- El buscador siempre a la vista: es lo que se usa en una agenda, y
         esconderlo detrás de un atajo o un botón lo vuelve invisible para quien
         no lo conoce.

         **Centrado en el hueco que queda**, no en la ventana entera. Va en el
         contenido de la barra —la única ranura que crece— con `m-auto`, que en
         un contenedor flexible reparte lo que sobra a los dos lados. Estaba en
         `centro`, que centra respecto de la ventana: con el icono de un lado y
         el estado, el botón de actualizar y los tres controles del otro, el
         medio de la ventana no es el medio del hueco, y el campo quedaba
         corrido a la derecha con la mitad izquierda de la barra vacía.

         `m-auto` y no `mx-auto` porque la barra también puede ir a un costado:
         ahí el eje del hueco es el vertical, y el margen automático en los dos
         ejes centra en el que corresponda sin preguntar cuál es.

         Es el `BarSearch` de la librería, que resuelve el único caso donde
         «siempre a la vista» no se puede cumplir: con la barra a un costado hay
         cuarenta y ocho píxeles de ancho y un campo de texto ahí no se lee ni
         se escribe. Ahí queda la lupa y el campo se abre al lado de la barra. -->
    <template #barra>
      <div class="m-auto flex items-center gap-2">
        <BarSearch
          v-model="consulta"
          :collapsed="narrow"
          :placeholder="t('lista.buscar')"
          :label="t('lista.buscar')" />
        <!-- Cuántos hay, que con una búsqueda escrita es cuántos coinciden. Es
             la única respuesta que da el buscador cuando no encuentra nada.

             Al lado del campo y contando para el centrado: lo que se ve como
             una sola cosa es «campo más número», y centrar sólo el campo deja
             al conjunto corrido. Con el ancho mínimo, pasar de «9 contactos» a
             «124 contactos» no mueve el campo; sin él, cada dígito lo corría
             medio carácter, que es por lo que este número colgaba aparte. -->
        <span
          v-if="!narrow"
          class="min-w-24 whitespace-nowrap text-tx-muted text-xs tabular-nums"
          aria-live="polite">{{ countLabel }}</span>
      </div>
    </template>

    <!-- Sin permiso para leer el almacén, la agenda queda vacía y sin salida:
         en vez de las columnas vacías, un estado que lo dice y el botón que
         vuelve a abrir el diálogo. Ocupa la fila entera, así que con la ventana
         angosta se ve igual —una sola cosa centrada— sin nada que apilar. -->
    <div v-if="sinPermiso" class="flex min-h-0 min-w-0 flex-1 items-center justify-center">
      <EmptyState
        icon="dialog-password"
        :title="t('permiso.titulo')"
        :note="t('permiso.descripcion')">
        <div class="flex min-w-0 flex-col items-center gap-2">
          <ActionButton
            variant="primary"
            :label="t('permiso.pedir')"
            :loading="pidiendoPermiso"
            @click="reclamarPermiso" />
          <!-- El motivo de que no se pudiera ni pedir —el techo, un recurso
               ajeno—, debajo del botón y anunciado para el lector de pantalla. -->
          <p v-if="avisoPermiso" class="max-w-prose text-body-s text-tx-muted" role="status">
            {{ avisoPermiso }}
          </p>
        </div>
      </EmptyState>
    </div>
    <!-- Las secciones separadas por aire y no por líneas: cada una es una
         superficie redondeada, como los paneles del escritorio. -->
    <template v-else>
      <AccountsPanel
        :accounts="cuentas"
        :notices="avisos"
        :sync-statuses="syncStatuses"
        :refresh-failures="refreshFailures"
        :address-books="addressBooks"
        :hidden-books="hiddenBooks"
        :categories="categories"
        :selected-categories="selectedCategories"
        v-bind="{ 'data-pane': 'accounts' }"
        :class="paneClass('accounts', pane)"
        @forward="go('list')"
        @toggle-book="toggleBook"
        @toggle-category="toggleCategory"
        @clear-categories="clearCategories" />
      <ContactList
        :contacts="visibles"
        :selected="elegido"
        :query="consulta"
        :loading="cargando"
        :count-label="narrow ? countLabel : ''"
        :wrap="narrow"
        v-bind="{ 'data-pane': 'list' }"
        :class="paneClass('list', pane)"
        @select="select"
        @back="go('accounts')" />
      <ContactDetail
        :contact="elegido"
        :is-own="own.isOwn(elegido)"
        v-bind="{ 'data-pane': 'detail' }"
        :class="paneClass('detail', pane)"
        @back="go('list')"
        @edit="openEdit"
        @remove="removeContact"
        @resolve="openConflict"
        @qr="openQr"
        @toggle-own="own.toggle" />
    </template>

    <!-- El aviso de que un borrado no se pudo: una barra al pie, que se cierra.
         No tapa la agenda, que sigue mostrando al contacto que volvió. -->
    <template #pie>
      <RevealTransition>
        <AlertMessage
          v-if="actionNotice"
          tone="error"
          variant="banner"
          icon="auto"
          dismissible
          :title="actionNotice"
          @close="actionNotice = ''" />
      </RevealTransition>
    </template>

    <!-- El formulario de crear o editar. Flota dentro de la ventana, así que es
         opaco (lo pone `DialogContent`); no cambia el formato de ninguna
         columna. -->
    <ContactForm
      v-model:open="formOpen"
      :contact="editing"
      :address-books="addressBooks"
      :saving="saving"
      :error="formError"
      @submit="submitForm" />

    <!-- El código QR de una tarjeta, con las casillas de qué va. Flota dentro de
         la ventana, así que es opaco (lo pone `DialogContent`). -->
    <QrDialog v-model:open="qrOpen" :contact="qrContact" />

    <OwnCardDialog
      v-model:open="ownOpen"
      :loading="ownLooking"
      :proposal="ownProposal"
      @confirm="confirmOwnProposal" />

    <!-- La comparación de un conflicto: la versión local y la del servidor, con
         «mantener la mía» y «tomar la del servidor». Flota dentro de la ventana,
         así que es opaca (lo pone `DialogContent`). -->
    <ConflictDialog
      v-model:open="conflictOpen"
      :contact="conflictContact"
      :load="getConflict"
      :resolve="resolveConflict" />
  </WindowAppLayout>
</template>
