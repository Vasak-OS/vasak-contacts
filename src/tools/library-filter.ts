/**
 * Elegir qué libretas se ven y filtrar por categoría (vasak-contacts#4).
 *
 * Aparte de los componentes y sin nada de Vue adentro, como `address-book.ts`:
 * son reglas que se equivocan en silencio —una libreta que se cree oculta y
 * sigue a la vista, una categoría que no junta a quien la tiene— y conviene
 * poder probarlas sin la ventana.
 *
 * ── Qué es local y qué no ────────────────────────────────────────────────────
 *
 * Qué libretas se ven es **preferencia de quien mira**, no un dato del servidor:
 * vive en el consumidor y se guarda en el navegador de la ventana. Las
 * categorías (`CATEGORIES` de la tarjeta) sí son dato; acá sólo se las junta y
 * se filtra por ellas. Asignarlas o quitarlas es escritura, y la escritura
 * todavía no existe (es el #2/#3).
 */

import type { Contact } from '@/tools/address-book';

/**
 * Una libreta de una cuenta, para la barra lateral.
 *
 * La identidad es la cuenta **más** el id de la libreta: dos cuentas pueden
 * tener una libreta con el mismo id —la «Contactos» de cada una—, y sin la
 * cuenta ocultar una ocultaría la otra.
 */
export interface AddressBook {
	accountId: string;
	accountName: string;
	id: string;
	name: string;
	/** La identidad, ya armada: `bookKey(accountId, id)`. */
	key: string;
}

/**
 * La identidad de una libreta. Con un separador que no aparece en un id ni en
 * una dirección: el byte cero. Pegar `cuenta + id` sin separador haría que
 * `a`+`b1` y `a1`+`b` fueran la misma clave.
 */
export function bookKey(accountId: string, id: string): string {
	return `${accountId}\u0000${id}`;
}

/** La identidad de la libreta en la que vive un contacto. */
export function contactBookKey(contact: Contact): string {
	return bookKey(contact.account_id, contact.address_book_id);
}

/**
 * Las libretas de todas las cuentas, en el orden en que llegaron y sin
 * repetir. Cada cuenta trae las suyas ya tagueadas con su id y su nombre.
 */
export function collectAddressBooks(
	accounts: { accountId: string; accountName: string; books: { url: string; nombre: string }[] }[]
): AddressBook[] {
	const out: AddressBook[] = [];
	const seen = new Set<string>();
	for (const account of accounts) {
		for (const book of account.books) {
			const key = bookKey(account.accountId, book.url);
			if (seen.has(key)) {
				continue;
			}
			seen.add(key);
			out.push({
				accountId: account.accountId,
				accountName: account.accountName,
				id: book.url,
				name: book.nombre,
				key,
			});
		}
	}
	return out;
}

/**
 * Los contactos de las libretas que **no** están ocultas.
 *
 * Lo que no pertenece a una libreta oculta se muestra, también lo que llegó sin
 * libreta (`address_book_id` vacío): esconder algo es una decisión, y una
 * libreta que la ventana no listó no es una decisión de esconder nada.
 */
export function filterByBooks(contacts: Contact[], hidden: Set<string>): Contact[] {
	if (hidden.size === 0) {
		return contacts;
	}
	return contacts.filter((contact) => !hidden.has(contactBookKey(contact)));
}

/**
 * Las categorías que aparecen en una lista de contactos, sin repetir y
 * ordenadas en el idioma de la sesión.
 *
 * Con el comparador del idioma y sin distinguir mayúsculas, como el orden de la
 * agenda: así «Trabajo» y «trabajo» no salen como dos, y los acentos no mandan
 * una categoría al final de la lista.
 */
export function collectCategories(contacts: Contact[], locale: string): string[] {
	const byLower = new Map<string, string>();
	for (const contact of contacts) {
		for (const category of contact.categories) {
			const trimmed = category.trim();
			if (trimmed && !byLower.has(trimmed.toLowerCase())) {
				byLower.set(trimmed.toLowerCase(), trimmed);
			}
		}
	}
	const collator = new Intl.Collator(locale, { sensitivity: 'base', numeric: true });
	return [...byLower.values()].sort((a, b) => collator.compare(a, b));
}

/**
 * Los contactos que tienen **alguna** de las categorías elegidas.
 *
 * «Alguna» y no «todas»: elegir «Trabajo» y «Amigos» muestra a quien esté en
 * cualquiera de las dos, que es lo que quiere decir marcar las dos en un filtro.
 * Sin ninguna elegida, están todos.
 */
export function filterByCategories(contacts: Contact[], selected: Set<string>): Contact[] {
	if (selected.size === 0) {
		return contacts;
	}
	return contacts.filter((contact) => contact.categories.some((c) => selected.has(c)));
}

/** Dónde se guarda, en el navegador de la ventana, qué libretas se ocultaron. */
const HIDDEN_BOOKS_KEY = 'vasak-contacts.hidden-address-books';

/**
 * Lee qué libretas estaban ocultas. Si no se puede —una ventana privada, el
 * almacenamiento bloqueado, un valor que alguien ensució—, no se oculta
 * ninguna: ver de más es mejor que esconder lo que la persona no pidió
 * esconder.
 */
export function loadHiddenBooks(): Set<string> {
	try {
		const raw = globalThis.localStorage?.getItem(HIDDEN_BOOKS_KEY);
		if (!raw) {
			return new Set();
		}
		const parsed: unknown = JSON.parse(raw);
		if (!Array.isArray(parsed)) {
			return new Set();
		}
		return new Set(parsed.filter((item): item is string => typeof item === 'string'));
	} catch {
		return new Set();
	}
}

/**
 * Guarda qué libretas se ocultaron. Si no se puede, se sigue sin guardar: el
 * filtro vale para esta sesión igual, sólo que no se recuerda para la próxima.
 */
export function saveHiddenBooks(hidden: Set<string>): void {
	try {
		globalThis.localStorage?.setItem(HIDDEN_BOOKS_KEY, JSON.stringify([...hidden]));
	} catch {
		// No recordar la preferencia no rompe nada que se esté viendo ahora.
	}
}
