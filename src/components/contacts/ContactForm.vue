<script lang="ts" setup>
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import {
	ActionButton,
	Dialog,
	DialogBody,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	FormGroup,
	SelectField,
	type SelectOption,
	TextArea,
	TextInput,
} from '@vasakgroup/vue-libvasak';
import { computed, reactive, ref, watch } from 'vue';
import type { Contact, LabeledValue } from '@/tools/address-book';
import {
	buildContactPatch,
	type ContactFormState,
	emptyForm,
	formFromContact,
} from '@/tools/contact-patch';
import type { AddressBook } from '@/tools/library-filter';

const props = defineProps<{
	open: boolean;
	/** El contacto a editar, o `null` para crear uno nuevo. */
	contact: Contact | null;
	/** Las libretas donde se puede crear, para elegir una al crear. */
	addressBooks: AddressBook[];
	/** Mientras se guarda: los botones se desactivan y el primario gira. */
	saving?: boolean;
	/** El error de guardar, ya traducido; vacío si no hubo. */
	error?: string;
}>();

const emit = defineEmits<{
	'update:open': [open: boolean];
	/** Guardar: todo lo que el comando necesita, con el parche ya armado. */
	submit: [
		payload: {
			accountId: string;
			addressBookId: string;
			payload: Record<string, unknown>;
			ifMatch: string;
		},
	];
}>();

const { t } = useI18n();

/** Si se está editando un contacto que ya existe. */
const editing = computed(() => props.contact !== null);

/** El estado del formulario, que se rearma cada vez que se abre. */
const form = reactive<ContactFormState>(emptyForm());
/** La libreta elegida para crear, como `AddressBook.key`. */
const targetKey = ref('');

/**
 * Las etiquetas, con su nombre traducido. El valor es la cadena canónica en
 * inglés que va al `TYPE` de la tarjeta; el nombre, lo que ve la persona.
 */
function labelOptions(values: string[]): SelectOption<string>[] {
	return values.map((value) => ({ value, label: t(`contact.labels.${value}`) }));
}
const EMAIL_LABELS = computed(() => labelOptions(['home', 'work', 'other']));
const PHONE_LABELS = computed(() => labelOptions(['cell', 'home', 'work', 'main', 'fax', 'other']));
const RELATION_LABELS = computed<SelectOption<string>[]>(() =>
	[
		'spouse',
		'child',
		'parent',
		'sibling',
		'friend',
		'partner',
		'assistant',
		'manager',
		'other',
	].map((value) => ({ value, label: t(`form.relations.${value}`) }))
);

/** Las libretas para elegir dónde crear, con el nombre de su cuenta. */
const bookOptions = computed<SelectOption<string>[]>(() =>
	props.addressBooks.map((book) => ({
		value: book.key,
		label: book.accountName ? `${book.name} · ${book.accountName}` : book.name,
	}))
);

/** Rearma el formulario al abrir: desde el contacto si se edita, vacío si no. */
watch(
	() => props.open,
	(open) => {
		if (!open) {
			return;
		}
		const base = props.contact ? formFromContact(props.contact) : emptyForm();
		form.fullName = base.fullName;
		form.organization = base.organization;
		form.note = base.note;
		form.emails = base.emails;
		form.phones = base.phones;
		form.related = base.related;
		// Al crear, se preselecciona la primera libreta (si hay una sola, queda
		// resuelta sin que la persona elija).
		targetKey.value = props.contact ? '' : (props.addressBooks[0]?.key ?? '');
	},
	{ immediate: true }
);

/** Agrega una fila vacía a una de las listas editables. */
function addRow(list: LabeledValue[], label: string) {
	list.push({ label, value: '' });
}

/** Saca una fila de una de las listas editables. */
function removeRow(list: LabeledValue[], index: number) {
	list.splice(index, 1);
}

/** Al crear hace falta una libreta; siempre hace falta un nombre. */
const targetBook = computed(
	() => props.addressBooks.find((b) => b.key === targetKey.value) ?? null
);
const canSubmit = computed(() => {
	if (props.saving) {
		return false;
	}
	if (!form.fullName.trim()) {
		return false;
	}
	return editing.value ? true : targetBook.value !== null;
});

function close() {
	emit('update:open', false);
}

function submit() {
	if (!canSubmit.value) {
		return;
	}
	if (editing.value && props.contact) {
		const id = Number(props.contact.store_id);
		emit('submit', {
			accountId: props.contact.account_id,
			addressBookId: props.contact.address_book_id,
			ifMatch: props.contact.etag,
			payload: buildContactPatch(form, {
				mode: 'edit',
				id: Number.isFinite(id) ? id : undefined,
			}),
		});
		return;
	}
	const book = targetBook.value;
	if (!book) {
		return;
	}
	emit('submit', {
		accountId: book.accountId,
		addressBookId: book.id,
		ifMatch: '',
		payload: buildContactPatch(form, { mode: 'create' }),
	});
}
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent size="wide">
      <DialogHeader closable :close-label="t('form.cancel')">
        <DialogTitle>{{ editing ? t('form.editTitle') : t('form.newTitle') }}</DialogTitle>
      </DialogHeader>

      <DialogBody>
        <!-- Contenedor de consultas: con la ventana angosta el formulario pone
             la etiqueta y el valor de cada fila en renglones separados, en vez
             de apretarlos uno al lado del otro. -->
        <form class="@container flex flex-col gap-4" @submit.prevent="submit">
          <FormGroup :label="t('form.name')" :html-for="'contact-name'">
            <template #default="{ id }">
              <TextInput :id="id" v-model="form.fullName" :placeholder="t('form.namePlaceholder')" />
            </template>
          </FormGroup>

          <FormGroup :label="t('form.organization')" :html-for="'contact-org'">
            <template #default="{ id }">
              <TextInput :id="id" v-model="form.organization" />
            </template>
          </FormGroup>

          <!-- Dónde crear. Sólo al crear y sólo si hay más de una libreta: con
               una sola ya queda resuelta y preguntar no agrega nada. -->
          <FormGroup
            v-if="!editing && bookOptions.length > 1"
            :label="t('form.addressBook')"
            :html-for="'contact-book'">
            <template #default="{ id }">
              <SelectField :id="id" v-model="targetKey" :options="bookOptions" />
            </template>
          </FormGroup>

          <!-- Correos: una fila por dirección, con su etiqueta. -->
          <fieldset class="flex flex-col gap-2">
            <legend class="font-title text-body-s text-tx-muted">{{ t('contact.emails') }}</legend>
            <div
              v-for="(email, i) in form.emails"
              :key="`email-${i}`"
              class="flex items-center gap-2 @max-[22rem]:flex-wrap">
              <SelectField
                v-model="email.label"
                :options="EMAIL_LABELS"
                class="w-28 shrink-0 @max-[22rem]:w-full" />
              <TextInput
                v-model="email.value"
                type="email"
                :ariaLabel="t('contact.emails')"
                :placeholder="t('form.emailPlaceholder')"
                class="min-w-0 flex-1 @max-[22rem]:basis-full" />
              <ActionButton
                variant="ghost"
                size="sm"
                icon="list-remove"
                icon-type="symbol"
                :label="t('form.remove')"
                @click="removeRow(form.emails, i)" />
            </div>
            <ActionButton
              variant="ghost"
              size="sm"
              icon="list-add"
              icon-type="symbol"
              :label="t('form.addEmail')"
              @click="addRow(form.emails, 'home')" />
          </fieldset>

          <!-- Teléfonos: igual que los correos. -->
          <fieldset class="flex flex-col gap-2">
            <legend class="font-title text-body-s text-tx-muted">{{ t('contact.phones') }}</legend>
            <div
              v-for="(phone, i) in form.phones"
              :key="`phone-${i}`"
              class="flex items-center gap-2 @max-[22rem]:flex-wrap">
              <SelectField
                v-model="phone.label"
                :options="PHONE_LABELS"
                class="w-28 shrink-0 @max-[22rem]:w-full" />
              <TextInput
                v-model="phone.value"
                :ariaLabel="t('contact.phones')"
                :placeholder="t('form.phonePlaceholder')"
                class="min-w-0 flex-1 @max-[22rem]:basis-full" />
              <ActionButton
                variant="ghost"
                size="sm"
                icon="list-remove"
                icon-type="symbol"
                :label="t('form.remove')"
                @click="removeRow(form.phones, i)" />
            </div>
            <ActionButton
              variant="ghost"
              size="sm"
              icon="list-add"
              icon-type="symbol"
              :label="t('form.addPhone')"
              @click="addRow(form.phones, 'cell')" />
          </fieldset>

          <!-- Relaciones: el vínculo en la etiqueta, el nombre en el valor. -->
          <fieldset class="flex flex-col gap-2">
            <legend class="font-title text-body-s text-tx-muted">{{ t('contact.relations') }}</legend>
            <div
              v-for="(relation, i) in form.related"
              :key="`relation-${i}`"
              class="flex items-center gap-2 @max-[22rem]:flex-wrap">
              <SelectField
                v-model="relation.label"
                :options="RELATION_LABELS"
                class="w-28 shrink-0 @max-[22rem]:w-full" />
              <TextInput
                v-model="relation.value"
                :ariaLabel="t('contact.relations')"
                :placeholder="t('form.relationPlaceholder')"
                class="min-w-0 flex-1 @max-[22rem]:basis-full" />
              <ActionButton
                variant="ghost"
                size="sm"
                icon="list-remove"
                icon-type="symbol"
                :label="t('form.remove')"
                @click="removeRow(form.related, i)" />
            </div>
            <ActionButton
              variant="ghost"
              size="sm"
              icon="list-add"
              icon-type="symbol"
              :label="t('form.addRelation')"
              @click="addRow(form.related, 'spouse')" />
          </fieldset>

          <FormGroup :label="t('contact.notes')" :html-for="'contact-note'">
            <template #default="{ id }">
              <TextArea :id="id" v-model="form.note" :rows="3" />
            </template>
          </FormGroup>

          <!-- El error de guardar, anunciado para el lector de pantalla. -->
          <p v-if="error" class="text-body-s text-status-error" role="alert">{{ error }}</p>
        </form>
      </DialogBody>

      <DialogFooter>
        <ActionButton variant="secondary" :label="t('form.cancel')" :disabled="saving" @click="close" />
        <ActionButton
          variant="primary"
          :label="editing ? t('form.save') : t('form.create')"
          :loading="saving"
          :disabled="!canSubmit"
          @click="submit" />
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
