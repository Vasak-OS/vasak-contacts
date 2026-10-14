<script lang="ts" setup>
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import {
	ActionButton,
	AlertMessage,
	Dialog,
	DialogBody,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	LoadingState,
	Panel,
	SectionHeading,
} from '@vasakgroup/vue-libvasak';
import { computed, ref, watch } from 'vue';
import type { Contact } from '@/tools/address-book';
import {
	type ConflictChoice,
	type ConflictDetail,
	formattedRaw,
	hasReadableFields,
	type ReadableContact,
	readableContact,
} from '@/tools/conflict';
import { isStorePermissionDenied } from '@/tools/store-permission';

const props = defineProps<{
	/** Si el diálogo está abierto. */
	open: boolean;
	/** El contacto en conflicto que se está resolviendo. */
	contact: Contact | null;
	/** Trae la comparación (la versión local y la del servidor). */
	load: (contact: Contact) => Promise<ConflictDetail>;
	/** Resuelve el conflicto y relee; devuelve el error crudo o `null`. */
	resolve: (contact: Contact, choice: ConflictChoice) => Promise<string | null>;
}>();

const emit = defineEmits<{ 'update:open': [value: boolean] }>();

const { t } = useI18n();

/** La comparación traída, mientras se mira el conflicto. */
const detail = ref<ConflictDetail | null>(null);
/** Mientras se trae la comparación. */
const loading = ref(false);
/** Si no se pudo traer la comparación, el texto ya traducido. */
const loadError = ref('');
/** Mientras se resuelve, para que no se pueda elegir dos veces. */
const resolving = ref(false);
/** Si resolver no se pudo, el texto ya traducido. */
const resolveError = ref('');

/** Traduce un error de un comando del almacén para mostrarlo. */
function errorText(error: string): string {
	return isStorePermissionDenied(error) ? t('conflict.denied') : t('conflict.error');
}

/** Trae la comparación del contacto en conflicto. */
async function fetchDetail() {
	const contact = props.contact;
	if (!contact) {
		return;
	}
	loading.value = true;
	loadError.value = '';
	resolveError.value = '';
	try {
		detail.value = await props.load(contact);
	} catch (e) {
		detail.value = null;
		loadError.value = errorText(String(e));
	} finally {
		loading.value = false;
	}
}

// Al abrirse, se trae la comparación; al cerrarse, se olvida lo traído para que
// la próxima vez no parpadee la de antes. `immediate` porque el diálogo puede
// nacer ya abierto (y si nace cerrado, la rama del `else` no hace nada).
watch(
	() => props.open,
	(open) => {
		if (open) {
			void fetchDetail();
		} else {
			detail.value = null;
			loadError.value = '';
			resolveError.value = '';
		}
	},
	{ immediate: true }
);

/** Un borrado local no tiene versión local que mostrar: «la mía» es borrarlo. */
const isDelete = computed(() => detail.value?.op === 'delete');

/** Si todavía no hay copia del servidor con qué comparar. */
const serverUnavailable = computed(() => detail.value?.server_unavailable ?? true);

/** Los pares nombre/valor de una versión, con la etiqueta ya traducida. */
function rowsOf(read: ReadableContact): { key: string; label: string; value: string }[] {
	const rows: { key: string; label: string; value: string }[] = [];
	if (read.name) {
		rows.push({ key: 'name', label: t('form.name'), value: read.name });
	}
	if (read.organization) {
		rows.push({ key: 'org', label: t('form.organization'), value: read.organization });
	}
	read.emails.forEach((value, i) => {
		rows.push({ key: `email-${i}`, label: t('contact.emails'), value });
	});
	read.phones.forEach((value, i) => {
		rows.push({ key: `phone-${i}`, label: t('contact.phones'), value });
	});
	return rows;
}

/** Una versión leída de su vCard; `null` cuando no hay crudo que leer. */
function versionOf(raw: string | null): ReadableContact | null {
	return raw ? readableContact(raw) : null;
}

const localRead = computed(() => versionOf(detail.value?.local ?? null));
const serverRead = computed(() => versionOf(detail.value?.server ?? null));

/** Resuelve con la elección y, si salió bien, cierra. */
async function choose(choice: ConflictChoice) {
	const contact = props.contact;
	if (!contact || resolving.value) {
		return;
	}
	resolving.value = true;
	resolveError.value = '';
	try {
		const error = await props.resolve(contact, choice);
		if (error) {
			resolveError.value = errorText(error);
			return;
		}
		emit('update:open', false);
	} finally {
		resolving.value = false;
	}
}
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <!-- La comparación flota dentro de la ventana, así que es opaca (lo pone
         `DialogContent`). `lg` porque son dos columnas de datos: con `sm` cada
         versión quedaba en una tira. -->
    <DialogContent size="lg">
      <DialogHeader>
        <DialogTitle>{{ t('conflict.title') }}</DialogTitle>
      </DialogHeader>
      <DialogBody>
        <p class="mb-3 text-body-m text-tx-muted">{{ t('conflict.explain') }}</p>

        <LoadingState v-if="loading" size="sm" :label="t('conflict.loading')" />

        <AlertMessage
          v-else-if="loadError"
          tone="error"
          variant="banner"
          icon="auto"
          :title="loadError" />

        <!-- Las dos versiones, una al lado de la otra cuando hay lugar y apiladas
             cuando no (consulta de contenedor, no punto de corte de pantalla). -->
        <div v-else-if="detail" class="@container">
          <div class="grid grid-cols-1 gap-3 @min-[32rem]:grid-cols-2">
            <!-- La mía -->
            <Panel as="section" padding="md" class="flex min-w-0 flex-col gap-2" v-bind="{ 'data-testid': 'mine' }">
              <SectionHeading as="h3" :title="t('conflict.mine')" />
              <p v-if="isDelete" class="text-body-m" data-testid="mine-delete">
                {{ t('conflict.mineDelete') }}
              </p>
              <template v-else-if="localRead && hasReadableFields(localRead)">
                <p v-for="row in rowsOf(localRead)" :key="row.key" class="min-w-0 break-words text-body-s">
                  <span class="text-tx-muted text-xs">{{ row.label }} · </span>{{ row.value }}
                </p>
              </template>
              <!-- Una tarjeta que no se pudo leer: el crudo formateado, para
                   poder comparar algo en vez de una ficha vacía. -->
              <pre
                v-else
                class="whitespace-pre-wrap break-words font-sans text-body-s text-tx-muted"
                data-testid="mine-raw">{{ formattedRaw(detail.local ?? '') }}</pre>
            </Panel>

            <!-- La del servidor -->
            <Panel as="section" padding="md" class="flex min-w-0 flex-col gap-2" v-bind="{ 'data-testid': 'theirs' }">
              <SectionHeading as="h3" :title="t('conflict.theirs')" />
              <template v-if="serverUnavailable">
                <p class="text-body-m text-tx-muted" data-testid="server-unavailable">
                  {{ t('conflict.serverUnavailable') }}
                </p>
                <ActionButton
                  class="self-start"
                  variant="secondary"
                  size="sm"
                  :label="t('conflict.retry')"
                  :loading="loading"
                  @click="fetchDetail" />
              </template>
              <template v-else-if="serverRead && hasReadableFields(serverRead)">
                <p v-for="row in rowsOf(serverRead)" :key="row.key" class="min-w-0 break-words text-body-s">
                  <span class="text-tx-muted text-xs">{{ row.label }} · </span>{{ row.value }}
                </p>
              </template>
              <pre
                v-else
                class="whitespace-pre-wrap break-words font-sans text-body-s text-tx-muted"
                data-testid="theirs-raw">{{ formattedRaw(detail.server ?? '') }}</pre>
            </Panel>
          </div>
        </div>

        <AlertMessage
          v-if="resolveError"
          class="mt-3"
          tone="error"
          variant="banner"
          icon="auto"
          :title="resolveError" />
      </DialogBody>
      <DialogFooter>
        <ActionButton variant="secondary" :label="t('form.cancel')" @click="emit('update:open', false)" />
        <ActionButton
          variant="secondary"
          :label="t('conflict.keepMine')"
          :disabled="resolving || loading || !!loadError"
          @click="choose('keep_local')" />
        <!-- Tomar la del servidor sólo cuando hay una copia con qué quedarse:
             sin ella no hay nada que tomar, y el botón confundiría. -->
        <ActionButton
          variant="primary"
          :label="t('conflict.takeTheirs')"
          :disabled="resolving || loading || !!loadError || serverUnavailable"
          @click="choose('take_server')" />
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
