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
	LoadingState,
} from '@vasakgroup/vue-libvasak';
import type { Contact } from '@/tools/address-book';
import { interpolar } from '@/tools/interpolar';

defineProps<{
	open: boolean;
	/** Mientras se le pregunta al servidor cuál es la tarjeta de la persona. */
	loading: boolean;
	/** La que el servidor dice que es, o `null` si no lo dice. */
	proposal: Contact | null;
}>();

const emit = defineEmits<{
	'update:open': [open: boolean];
	/** La persona confirmó que la propuesta es ella. */
	confirm: [];
}>();

const { t } = useI18n();
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent size="sm">
      <DialogHeader closable :close-label="t('form.cancel')">
        <DialogTitle>{{ t('ownCard.title') }}</DialogTitle>
      </DialogHeader>

      <DialogBody>
        <LoadingState v-if="loading" :label="t('ownCard.looking')" />
        <!-- La propuesta, para confirmar: el servidor dice cuál es, pero la
             marca es de la persona. -->
        <p v-else-if="proposal" class="text-body-m" data-testid="own-proposal">
          {{ interpolar(t('ownCard.proposal'), proposal.name || t('lista.sinNombre')) }}
        </p>
        <!-- Sin propuesta: el respaldo es elegirla a mano desde su ficha. -->
        <p v-else class="text-body-m" data-testid="own-none">{{ t('ownCard.none') }}</p>
      </DialogBody>

      <DialogFooter>
        <ActionButton variant="secondary" :label="t('ownCard.close')" @click="emit('update:open', false)" />
        <ActionButton
          v-if="proposal && !loading"
          variant="primary"
          :label="t('ownCard.confirm')"
          @click="emit('confirm')" />
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
