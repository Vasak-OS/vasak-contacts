/**
 * Qué campos de un contacto van al código QR de su tarjeta (vasak-contacts#10).
 *
 * Aparte de la ventana y sin nada de Vue adentro, como `contact-patch.ts`: es la
 * regla que se equivoca **en silencio**. Compartir la tarjeta propia es dar
 * datos personales a alguien que está enfrente, y lo que sale en el código tiene
 * que ser exactamente lo que las casillas mostraban: un teléfono de más no se
 * nota en el QR, sólo lo nota quien lo escanea.
 *
 * Lo que va, por tope: el nombre, hasta dos teléfonos, un correo y la
 * organización. **Nunca la foto**: no entra en un código, y no está en
 * `QrCard` ni hay forma de pedirla. La vCard la arma el programa
 * (`vcard::export`) a partir de este objeto.
 */

import type { Contact } from '@/tools/address-book';

/** Cuántos teléfonos entran. Es el mismo tope del programa (`MAX_PHONES`). */
export const QR_MAX_PHONES = 2;
/** Cuántos correos entran. */
export const QR_MAX_EMAILS = 1;

/** Las casillas: qué se marcó de cada campo. Los índices son los de la ficha. */
export interface QrSelection {
	name: boolean;
	/** Posiciones, dentro de `contact.phones`, de los teléfonos marcados. */
	phones: number[];
	/** Posiciones, dentro de `contact.emails`, de los correos marcados. */
	emails: number[];
	organization: boolean;
}

/** Lo que se le manda al programa: la forma de `vcard::QrCard`. */
export interface QrCard {
	name: string;
	phones: string[];
	email: string | null;
	organization: string;
}

/** La clave de traducción del error de generar, o la genérica si no se conoce. */
export function qrErrorKey(error: string): string {
	switch (error) {
		case 'qr-empty':
			return 'qr.empty';
		case 'qr-too-long':
			return 'qr.tooLong';
		default:
			return 'qr.error';
	}
}

/** Las posiciones de los primeros `max` valores que no están vacíos. */
function firstFilled(values: { value: string }[], max: number): number[] {
	const out: number[] = [];
	values.forEach((entry, i) => {
		if (out.length < max && entry.value.trim()) {
			out.push(i);
		}
	});
	return out;
}

/**
 * Lo que viene marcado al abrir: lo que dice el alcance —el nombre, los dos
 * primeros teléfonos, el primer correo y la organización—, y sólo lo que existe.
 * La persona lo ve y lo cambia antes de generar nada.
 */
export function defaultSelection(contact: Contact): QrSelection {
	return {
		name: contact.name.trim().length > 0,
		phones: firstFilled(contact.phones, QR_MAX_PHONES),
		emails: firstFilled(contact.emails, QR_MAX_EMAILS),
		organization: contact.organization.trim().length > 0,
	};
}

/**
 * Lo marcado, ya recortado, para el programa. Los índices que no existen o que
 * apuntan a un valor vacío se ignoran, y los que pasan del tope no entran: el
 * resultado nunca lleva más de lo que las casillas permiten.
 */
export function buildQrCard(contact: Contact, selection: QrSelection): QrCard {
	const pick = (values: { value: string }[], indexes: number[], max: number) =>
		[...new Set(indexes)]
			.sort((a, b) => a - b)
			.map((i) => values[i]?.value.trim() ?? '')
			.filter((value) => value.length > 0)
			.slice(0, max);

	return {
		name: selection.name ? contact.name.trim() : '',
		phones: pick(contact.phones, selection.phones, QR_MAX_PHONES),
		email: pick(contact.emails, selection.emails, QR_MAX_EMAILS)[0] ?? null,
		organization: selection.organization ? contact.organization.trim() : '',
	};
}

/** Si no hay nada que compartir: el botón de generar no tiene qué hacer. */
export function isQrCardEmpty(card: QrCard): boolean {
	return !card.name && !card.organization && card.email === null && card.phones.length === 0;
}
