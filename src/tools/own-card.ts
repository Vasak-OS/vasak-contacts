/**
 * Cuál es la tarjeta de la persona («éste soy yo», vasak-contacts#10).
 *
 * Un `Contact` no lleva esa marca —la tarjeta no la tiene y el almacén no la
 * guarda—, así que es una **preferencia de la ventana**: se recuerda en el
 * navegador, como las libretas ocultas (`library-filter.ts`). Se guarda la
 * identidad del contacto (`url`: la cuenta y dónde vive la tarjeta), no el
 * número de la base ni el `UID`, que pueden cambiar o repetirse entre cuentas.
 *
 * La propuesta sale de `CARDDAV:principal-address` del servidor (la lee
 * `own_card_address`, de sólo lectura); si no la dice, la persona elige.
 */

import type { Contact } from '@/tools/address-book';

const OWN_CARD_KEY = 'vasak-contacts.own-card';

/** Con qué se reconoce al contacto marcado: su identidad, o el `UID` si no hay. */
export function ownCardKey(contact: Contact): string {
	return contact.url || contact.uid;
}

/** El contacto marcado entre los que hay, o `null` si no está (se borró, otra cuenta). */
export function findOwnCard(contacts: Contact[], key: string): Contact | null {
	if (!key) {
		return null;
	}
	return contacts.find((contact) => ownCardKey(contact) === key) ?? null;
}

/**
 * Recuerda cuál es la tarjeta propia. Si no se puede guardar se sigue igual: la
 * marca vale para esta sesión, sólo que no se recuerda para la próxima.
 */
export function saveOwnCardKey(key: string): void {
	try {
		if (key) {
			globalThis.localStorage?.setItem(OWN_CARD_KEY, key);
		} else {
			globalThis.localStorage?.removeItem(OWN_CARD_KEY);
		}
	} catch {
		// No recordar la marca no rompe nada de lo que se está viendo.
	}
}

/** La tarjeta propia recordada, o vacío si no hay o no se puede leer. */
export function loadOwnCardKey(): string {
	try {
		return globalThis.localStorage?.getItem(OWN_CARD_KEY) ?? '';
	} catch {
		return '';
	}
}

/**
 * El camino de una dirección, sin el servidor y decodificado, para comparar
 * una `https://…/ana.vcf` con la `href` que guarda la identidad de un contacto,
 * que puede venir entera o sólo como camino.
 */
function pathOf(address: string): string {
	let path = address;
	try {
		path = new URL(address, 'https://invalid.invalid/').pathname;
	} catch {
		// Una dirección que no se puede leer se compara tal cual.
	}
	try {
		return decodeURI(path);
	} catch {
		return path;
	}
}

/**
 * Lo que está después de la cuenta en la identidad de un contacto del almacén
 * (`store:{cuenta}:{dónde}`); lo que no tiene esa forma —una lectura que fue al
 * servidor— es la dirección tal cual.
 */
function whereOf(identity: string): string {
	const match = /^store:[^:]*:(.*)$/s.exec(identity);
	return match ? (match[1] ?? '') : identity;
}

/**
 * El contacto al que apunta la tarjeta que dice el servidor, de esa cuenta.
 *
 * Se compara el **camino**, porque el almacén guarda la `href` a veces entera y
 * a veces relativa. Una sub-tarjeta de un recurso con varias (`…#1`) no cuenta:
 * la dirección es la del recurso y se queda con la primera.
 */
export function proposedOwnCard(
	contacts: Contact[],
	accountId: string,
	address: string
): Contact | null {
	const wanted = pathOf(address);
	return (
		contacts.find(
			(contact) =>
				contact.account_id === accountId &&
				!/#\d+$/.test(contact.url) &&
				pathOf(whereOf(contact.url)) === wanted
		) ?? null
	);
}
