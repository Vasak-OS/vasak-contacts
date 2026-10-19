<script lang="ts" setup>
import { invoke } from '@tauri-apps/api/core';
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import {
	ActionButton,
	Checkbox,
	Dialog,
	DialogBody,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	SectionHeading,
} from '@vasakgroup/vue-libvasak';
import { computed, reactive, ref, watch } from 'vue';
import type { Contact } from '@/tools/address-book';
import { labelKey } from '@/tools/contact-fields';
import {
	buildQrCard,
	defaultSelection,
	isQrCardEmpty,
	QR_MAX_EMAILS,
	QR_MAX_PHONES,
	type QrSelection,
	qrErrorKey,
} from '@/tools/qr-card';

const props = defineProps<{
	open: boolean;
	/** De quién es la tarjeta que se comparte. */
	contact: Contact | null;
}>();

const emit = defineEmits<{ 'update:open': [open: boolean] }>();

const { t } = useI18n();

/** Las casillas. Se rearman al abrir, desde lo que dice el contacto. */
const selection = reactive<QrSelection>({
	name: false,
	phones: [],
	emails: [],
	organization: false,
});
/** El código ya generado, como SVG; vacío hasta que se aprieta «Generar». */
const svg = ref('');
const error = ref('');
const generating = ref(false);

watch(
	() => [props.open, props.contact] as const,
	([open, contact]) => {
		svg.value = '';
		error.value = '';
		if (open && contact) {
			Object.assign(selection, defaultSelection(contact));
		}
	},
	{ immediate: true }
);

/**
 * Tocar una casilla borra el código: lo que se ve en pantalla es siempre lo que
 * las casillas dicen ahora, nunca un código viejo con otra selección.
 */
watch(selection, () => {
	svg.value = '';
	error.value = '';
});

const card = computed(() => (props.contact ? buildQrCard(props.contact, selection) : null));
const canGenerate = computed(() => card.value !== null && !isQrCardEmpty(card.value));

/** Marca o desmarca la posición `index` de una de las listas de la selección. */
function toggle(list: number[], index: number, checked: boolean) {
	const at = list.indexOf(index);
	if (checked && at < 0) {
		list.push(index);
	} else if (!checked && at >= 0) {
		list.splice(at, 1);
	}
}

/** Sin lugar para otra casilla: se destilda una antes de marcar la siguiente. */
function full(list: number[], index: number, max: number): boolean {
	return !list.includes(index) && list.length >= max;
}

/** La etiqueta de un dato (casa, trabajo…), traducida si es de las conocidas. */
function labelText(label: string): string | undefined {
	if (!label) {
		return undefined;
	}
	const key = labelKey(label);
	return key ? t(key) : label;
}

async function generate() {
	if (!card.value || !canGenerate.value || generating.value) {
		return;
	}
	generating.value = true;
	error.value = '';
	try {
		svg.value = await invoke<string>('generate_vcard_qr', { card: card.value });
	} catch (e) {
		svg.value = '';
		error.value = t(qrErrorKey(String(e)));
	} finally {
		generating.value = false;
	}
}
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent size="wide">
      <DialogHeader closable :close-label="t('form.cancel')">
        <DialogTitle>{{ t('qr.title') }}</DialogTitle>
      </DialogHeader>

      <DialogBody>
        <!-- Contenedor de consultas: con la ventana angosta las casillas y el
             código van en una sola columna; desde 30rem, lado a lado. -->
        <div v-if="contact" class="@container flex min-w-0 flex-col gap-4">
          <p class="text-body-s text-tx-muted">{{ t('qr.explain') }}</p>

          <div class="flex min-w-0 flex-col gap-4 @min-[30rem]:flex-row">
            <fieldset class="flex min-w-0 flex-1 flex-col gap-3" data-testid="qr-fields">
              <legend class="sr-only">{{ t('qr.fields') }}</legend>

              <Checkbox
                v-if="contact.name.trim()"
                v-model="selection.name"
                :label="contact.name"
                :description="t('qr.name')" />

              <div v-if="contact.phones.some((p) => p.value.trim())" class="flex flex-col gap-1">
                <SectionHeading as="h3" :title="t('contact.phones')" />
                <template v-for="(phone, i) in contact.phones" :key="`${i}-${phone.value}`">
                  <Checkbox
                    v-if="phone.value.trim()"
                    :model-value="selection.phones.includes(i)"
                    :disabled="full(selection.phones, i, QR_MAX_PHONES)"
                    :label="phone.value"
                    :description="labelText(phone.label)"
                    @update:model-value="toggle(selection.phones, i, $event)" />
                </template>
              </div>

              <div v-if="contact.emails.some((e) => e.value.trim())" class="flex flex-col gap-1">
                <SectionHeading as="h3" :title="t('contact.emails')" />
                <template v-for="(email, i) in contact.emails" :key="`${i}-${email.value}`">
                  <Checkbox
                    v-if="email.value.trim()"
                    :model-value="selection.emails.includes(i)"
                    :disabled="full(selection.emails, i, QR_MAX_EMAILS)"
                    :label="email.value"
                    :description="labelText(email.label)"
                    @update:model-value="toggle(selection.emails, i, $event)" />
                </template>
              </div>

              <Checkbox
                v-if="contact.organization.trim()"
                v-model="selection.organization"
                :label="contact.organization"
                :description="t('form.organization')" />

              <!-- Lo que **nunca** va, dicho: no es una casilla que se pueda marcar. -->
              <p class="text-tx-muted text-xs">{{ t('qr.never') }}</p>
            </fieldset>

            <div class="flex min-w-0 flex-1 flex-col items-center justify-center gap-2">
              <!-- El código es del programa y lleva sólo rectángulos: no hay
                   nada de afuera adentro. Sin colores propios: los módulos van
                   en `currentColor`. Oscuro sobre claro **siempre**, también en
                   el esquema oscuro —las cámaras leen mal el invertido—, así que
                   usa las variables del esquema claro (`--text-main`,
                   `--ui-background`) y no las que cambian con el modo. -->
              <div
                v-if="svg"
                class="aspect-square w-full max-w-64 rounded-corner-m bg-[var(--ui-background)] p-1 text-[var(--text-main)] [&>svg]:h-full [&>svg]:w-full"
                role="img"
                :aria-label="t('qr.alt')"
                data-testid="qr-code"
                v-html="svg" />
              <p v-else class="text-center text-body-s text-tx-muted" data-testid="qr-placeholder">
                {{ t('qr.placeholder') }}
              </p>
              <p v-if="error" class="text-body-s text-status-error" role="alert">{{ error }}</p>
            </div>
          </div>
        </div>
      </DialogBody>

      <DialogFooter>
        <ActionButton variant="secondary" :label="t('qr.close')" @click="emit('update:open', false)" />
        <ActionButton
          variant="primary"
          icon="view-barcode-qr"
          icon-type="symbol"
          :label="t('qr.generate')"
          :loading="generating"
          :disabled="!canGenerate"
          @click="generate" />
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
