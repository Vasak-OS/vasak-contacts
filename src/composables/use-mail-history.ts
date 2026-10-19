import { invoke } from '@tauri-apps/api/core';
import { ref, watch } from 'vue';
import type { Contact } from '@/tools/address-book';
import {
	collectHistory,
	historyAddresses,
	loadMailHistoryEnabled,
	MAIL_HISTORY_LIMIT,
	type MailHistoryState,
	type MailInteraction,
	saveMailHistoryEnabled,
} from '@/tools/mail-history';

/**
 * El historial de correo del contacto que se está viendo.
 *
 * Apagado por omisión: con la sección apagada **no se consulta nada**. Encendida,
 * se pregunta al abrir cada ficha y no se guarda la respuesta. La primera vez el
 * servicio abre el diálogo del permiso `store.email.interactions`; si la persona
 * dice que no, la sección lo dice (`denied`) y no vuelve a preguntar hasta que
 * se pida el permiso de nuevo.
 */
export function useMailHistory(contact: () => Contact | null) {
	const enabled = ref(loadMailHistoryEnabled());
	const state = ref<MailHistoryState>(enabled.value ? 'idle' : 'off');
	const items = ref<MailInteraction[]>([]);
	/** La marca de por qué no se pudo volver a pedir el permiso, o vacío. */
	const reaskError = ref('');
	const asking = ref(false);
	/** Cuál es la consulta vigente: una ficha que se cambió descarta la anterior. */
	let current = 0;
	/** El «no» ya está dado: no se le pregunta otra vez con cada ficha. */
	let denied = false;

	async function load() {
		const mine = ++current;
		items.value = [];
		const c = contact();
		const addresses = c?.account_id ? historyAddresses(c.emails) : [];
		if (!enabled.value) {
			state.value = 'off';
			return;
		}
		if (addresses.length === 0) {
			state.value = 'idle';
			return;
		}
		if (denied) {
			state.value = 'denied';
			return;
		}
		state.value = 'loading';
		const accountId = c?.account_id ?? '';
		const result = await collectHistory(addresses, (address) =>
			invoke<MailInteraction[]>('mail_history', {
				accountId,
				address,
				limit: MAIL_HISTORY_LIMIT,
			})
		);
		if (mine !== current) {
			return;
		}
		denied = result.state === 'denied';
		state.value = result.state;
		items.value = result.items;
	}

	watch(
		() => {
			const c = contact();
			return [enabled.value, c?.uid, c?.account_id, historyAddresses(c?.emails ?? []).join('\n')];
		},
		load,
		{ immediate: true }
	);

	/** Enciende o apaga la sección, y lo recuerda. */
	function setEnabled(on: boolean) {
		enabled.value = on;
		saveMailHistoryEnabled(on);
	}

	/**
	 * Vuelve a pedir el permiso y relee. Si no se pudo ni pedir (el techo de uno
	 * por minuto, un recurso ajeno) deja el motivo en `reaskError`.
	 */
	async function askAgain() {
		if (asking.value) {
			return;
		}
		asking.value = true;
		reaskError.value = '';
		try {
			await invoke('request_mail_history_permission_again');
		} catch (e) {
			reaskError.value = String(e);
		}
		denied = false;
		try {
			await load();
		} finally {
			asking.value = false;
		}
	}

	return { enabled, state, items, reaskError, asking, setEnabled, askAgain };
}
