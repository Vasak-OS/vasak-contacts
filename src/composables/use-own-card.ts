import { invoke } from '@tauri-apps/api/core';
import { computed, ref } from 'vue';
import type { Contact } from '@/tools/address-book';
import {
	findOwnCard,
	loadOwnCardKey,
	ownCardKey,
	proposedOwnCard,
	saveOwnCardKey,
} from '@/tools/own-card';

/**
 * La tarjeta propia: cuál es y cómo se propone (vasak-contacts#10).
 *
 * La marca es una preferencia de la ventana (`tools/own-card.ts`); la propuesta
 * sale de lo que dice el servidor de cada cuenta. Cualquier error al preguntarle
 * al servidor —sin permiso, sin red, un servidor que no lo contesta— es «sin
 * propuesta»: la persona elige ella, que es el respaldo y siempre funciona.
 */
export function useOwnCard(contacts: () => Contact[], accountIds: () => string[]) {
	const key = ref(loadOwnCardKey());

	/** El contacto marcado, o `null` si no hay o ya no está en la agenda. */
	const ownCard = computed(() => findOwnCard(contacts(), key.value));

	function isOwn(contact: Contact | null): boolean {
		return contact !== null && key.value !== '' && ownCardKey(contact) === key.value;
	}

	function mark(contact: Contact) {
		key.value = ownCardKey(contact);
		saveOwnCardKey(key.value);
	}

	function unmark() {
		key.value = '';
		saveOwnCardKey('');
	}

	/** Marca el contacto, o lo desmarca si ya era la tarjeta propia. */
	function toggle(contact: Contact) {
		if (isOwn(contact)) {
			unmark();
		} else {
			mark(contact);
		}
	}

	/**
	 * El contacto que el servidor dice que es la persona, de la primera cuenta
	 * que lo diga y cuya tarjeta esté en la agenda; `null` si ninguna.
	 */
	async function propose(): Promise<Contact | null> {
		for (const accountId of accountIds()) {
			try {
				const address = await invoke<string | null>('own_card_address', { accountId });
				const found = address ? proposedOwnCard(contacts(), accountId, address) : null;
				if (found) {
					return found;
				}
			} catch (e) {
				// Sin propuesta de esa cuenta: se prueba la siguiente o se elige a mano.
				console.error('no se pudo leer la tarjeta propia', e);
			}
		}
		return null;
	}

	return { ownCard, isOwn, mark, unmark, toggle, propose };
}
