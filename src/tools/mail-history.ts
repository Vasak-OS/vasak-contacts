/**
 * El historial de correo de la ficha (vasak-contacts#11, tramo correo).
 *
 * Aparte de los componentes y sin Vue ni Tauri adentro: lo que se arma acá es
 * qué se pregunta (las direcciones del contacto), cómo se junta lo que
 * contesta el servicio y en qué estado queda la sección. Se consulta al abrir la
 * ficha y **no se guarda**: sin base de interacciones no hay nada que filtrar.
 */

import type { LabeledValue } from '@/tools/address-book';
import { isStorePermissionDenied } from '@/tools/store-permission';

/** Un mensaje del historial, como lo entrega `MessagesWith`: sin cuerpo. */
export interface MailInteraction {
	uid: number;
	/** Segundos desde 1970; 0 si el mensaje no traía fecha. */
	date_ts: number;
	subject: string;
	from_addr: string;
}

/** Cuántos mensajes muestra la ficha. */
export const MAIL_HISTORY_LIMIT = 10;
/** Cuántas direcciones del contacto se consultan como mucho. */
export const MAX_HISTORY_ADDRESSES = 5;

/**
 * Cómo queda la sección: `off` (apagada, no se consulta nada), `idle` (sin qué
 * consultar), `loading`, `ready` (con o sin mensajes), `denied` (la persona dijo
 * que no) o `unavailable` (la cuenta no tiene correo, o el servicio no
 * contesta).
 */
export type MailHistoryState = 'off' | 'idle' | 'loading' | 'ready' | 'denied' | 'unavailable';

const ENABLED_KEY = 'vasak-contacts.mail-history';

/**
 * Si la persona encendió el historial. **Apagado por omisión**: lo más
 * conservador con la privacidad, y también lo que queda si el navegador de la
 * ventana no deja leer (modo privado, datos bloqueados).
 */
export function loadMailHistoryEnabled(): boolean {
	try {
		return globalThis.localStorage?.getItem(ENABLED_KEY) === '1';
	} catch {
		return false;
	}
}

/** Recuerda si está encendido. Si no se puede guardar, vale para esta sesión. */
export function saveMailHistoryEnabled(enabled: boolean): void {
	try {
		if (enabled) {
			globalThis.localStorage?.setItem(ENABLED_KEY, '1');
		} else {
			globalThis.localStorage?.removeItem(ENABLED_KEY);
		}
	} catch {
		// No recordarlo no rompe nada de lo que se ve ahora.
	}
}

/**
 * Las direcciones que se le preguntan al servicio: recortadas, sin repetir
 * (sin distinguir mayúsculas), sólo las que tienen arroba, y con tope.
 */
export function historyAddresses(emails: LabeledValue[]): string[] {
	const seen = new Set<string>();
	const out: string[] = [];
	for (const email of emails) {
		const address = email.value.trim();
		const key = address.toLowerCase();
		if (!address.includes('@') || seen.has(key)) {
			continue;
		}
		seen.add(key);
		out.push(address);
		if (out.length === MAX_HISTORY_ADDRESSES) {
			break;
		}
	}
	return out;
}

/**
 * Junta las listas de varias direcciones: más nuevos primero, sin repetir un
 * mismo mensaje que coincidió por dos direcciones, y acotado a `limit`.
 */
export function mergeHistory(lists: MailInteraction[][], limit: number): MailInteraction[] {
	const seen = new Set<string>();
	const merged: MailInteraction[] = [];
	for (const item of lists.flat()) {
		const key = `${item.uid}|${item.date_ts}|${item.subject}`;
		if (!seen.has(key)) {
			seen.add(key);
			merged.push(item);
		}
	}
	return merged.sort((a, b) => b.date_ts - a.date_ts).slice(0, Math.max(0, limit));
}

/**
 * Pregunta por cada dirección, de a una, y deja la sección en su estado.
 *
 * De a una y no en paralelo: la primera vez el servicio abre el diálogo del
 * permiso, y varias preguntas a la vez abrirían varios. Un «no» corta ahí; si
 * todas fallan por otro motivo —la cuenta no tiene correo, el servicio no
 * contesta— queda `unavailable`, y si falla sólo alguna se muestra lo que salió.
 */
export async function collectHistory(
	addresses: string[],
	fetchOne: (address: string) => Promise<MailInteraction[]>,
	limit: number = MAIL_HISTORY_LIMIT
): Promise<{ state: 'ready' | 'denied' | 'unavailable'; items: MailInteraction[] }> {
	const lists: MailInteraction[][] = [];
	let failures = 0;
	for (const address of addresses) {
		try {
			lists.push(await fetchOne(address));
		} catch (error) {
			if (isStorePermissionDenied(error)) {
				return { state: 'denied', items: [] };
			}
			failures += 1;
		}
	}
	if (addresses.length > 0 && failures === addresses.length) {
		return { state: 'unavailable', items: [] };
	}
	return { state: 'ready', items: mergeHistory(lists, limit) };
}

/** La fecha de un mensaje en el idioma de la sesión, o vacío si no traía. */
export function formatInteractionDate(timestamp: number, locale: string): string {
	if (!Number.isFinite(timestamp) || timestamp <= 0) {
		return '';
	}
	try {
		return new Intl.DateTimeFormat(locale || undefined, { dateStyle: 'medium' }).format(
			new Date(timestamp * 1000)
		);
	} catch {
		return new Date(timestamp * 1000).toISOString().slice(0, 10);
	}
}
