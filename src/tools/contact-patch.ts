/**
 * Arma el parche JSON para crear o editar un contacto (vasak-contacts#2/#3).
 *
 * Aparte del formulario y sin nada de Vue adentro, como `address-book.ts`: es
 * la regla que se equivoca **en silencio** —un campo que se manda cuando no se
 * tocó pisa lo que había, uno que se omite cuando se vació lo deja puesto— y la
 * escritura conservadora (#3) depende de acertarla.
 *
 * ── La forma del parche (A0, en main) ────────────────────────────────────────
 *
 * Cada campo: **ausente no toca, `null` saca, un valor fija**. El parche sólo
 * nombra los seis campos que el formulario maneja; lo demás de la tarjeta
 * —direcciones, fechas, foto, redes— no se nombra y por eso **no se toca**. Eso
 * es la escritura conservadora: la ventana no reescribe lo que no muestra.
 *
 * - `full_name`, `organization`, `note`: texto; vacío saca (`null`).
 * - `emails`, `phones`: lista de `{ value, label? }`; vacía saca todas (`null`).
 * - `related`: lista de `{ label, value }`.
 *
 * ── `related` y por qué se omite al editar ───────────────────────────────────
 *
 * El lado de lectura todavía no trae las relaciones, así que al abrir una ficha
 * para editarla el formulario no las puede precargar. Mandar `related: null`
 * entonces borraría las que la persona tiene sin que las haya visto nunca. Por
 * eso, al **editar**, una lista de relaciones vacía se **omite** (no toca); al
 * **crear** no hay nada que perder, y vacía se manda `null`. Cuando el PR que
 * suma `sync_state` a los DTOs traiga también las relaciones, esto deja de hacer
 * falta.
 */

import type { LabeledValue } from '@/tools/address-book';

/** Si el parche crea un contacto nuevo o edita uno que ya existe. */
export type PatchMode = 'create' | 'edit';

/** Lo que el formulario junta para armar el parche. */
export interface ContactFormState {
	fullName: string;
	organization: string;
	note: string;
	emails: LabeledValue[];
	phones: LabeledValue[];
	/** Relaciones: `label` es el vínculo (cónyuge, hijo…) y `value` el nombre. */
	related: LabeledValue[];
}

/** Un contacto vacío para el formulario, con lo que se le pase encima. */
export function emptyForm(overrides: Partial<ContactFormState> = {}): ContactFormState {
	return {
		fullName: '',
		organization: '',
		note: '',
		emails: [],
		phones: [],
		related: [],
		...overrides,
	};
}

/** El texto fijado, o `null` para sacarlo: un campo vacío borra lo que hubiera. */
function text(value: string): string | null {
	const trimmed = value.trim();
	return trimmed.length > 0 ? trimmed : null;
}

/** Un ítem del parche: el valor y, si la hay, la etiqueta. */
interface PatchItem {
	value: string;
	label?: string;
}

/** Deja un ítem sólo si tiene valor; recorta valor y etiqueta. */
function itemOf(entry: LabeledValue): PatchItem | null {
	const value = entry.value.trim();
	if (!value) {
		return null;
	}
	const label = entry.label.trim();
	return label ? { value, label } : { value };
}

/** La lista fijada, o `null` para sacarla toda; sin los ítems sin valor. */
function list(entries: LabeledValue[]): PatchItem[] | null {
	const items = entries.map(itemOf).filter((item): item is PatchItem => item !== null);
	return items.length > 0 ? items : null;
}

/** Lo que se pasa para armar el parche: el modo y, al editar, el número local. */
export interface PatchTarget {
	mode: PatchMode;
	/** El `store_id` del contacto, como número, cuando se edita. */
	id?: number;
}

/**
 * El parche JSON a partir de lo que el formulario junta.
 *
 * Al editar lleva el `id` (el número local); al crear no. Las relaciones siguen
 * la regla de arriba: vacías se omiten al editar y se mandan `null` al crear.
 */
export function buildContactPatch(
	form: ContactFormState,
	target: PatchTarget
): Record<string, unknown> {
	const patch: Record<string, unknown> = {
		full_name: text(form.fullName),
		organization: text(form.organization),
		note: text(form.note),
		emails: list(form.emails),
		phones: list(form.phones),
	};

	const related = list(form.related);
	if (related !== null) {
		patch.related = related;
	} else if (target.mode === 'create') {
		// Al crear no hay relaciones que perder: vacías se sacan.
		patch.related = null;
	}
	// Al editar, vacías se omiten: el formulario no las pudo precargar.

	if (target.mode === 'edit' && target.id !== undefined) {
		patch.id = target.id;
	}

	return patch;
}

/**
 * El formulario a partir de un contacto que se va a editar.
 *
 * Precarga lo que el lado de lectura sí trae; las relaciones quedan vacías
 * porque todavía no llegan (ver arriba).
 */
export function formFromContact(contact: {
	name: string;
	organization: string;
	notes: string;
	emails: LabeledValue[];
	phones: LabeledValue[];
}): ContactFormState {
	return {
		fullName: contact.name,
		organization: contact.organization,
		note: contact.notes,
		// Copias, para no mutar lo que vino del programa al editar las filas.
		emails: contact.emails.map((e) => ({ label: e.label, value: e.value })),
		phones: contact.phones.map((p) => ({ label: p.label, value: p.value })),
		related: [],
	};
}
