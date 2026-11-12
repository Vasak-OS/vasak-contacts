import type { Contact } from './address-book';
import { ownCardKey } from './own-card';

/**
 * Fusión **local** de tarjetas repetidas: unir en la ventana lo que es la misma
 * persona, sin tocar el servidor. Es reversible —se deshace y vuelven las
 * tarjetas como estaban— y no pierde nada: todos los correos y teléfonos, las
 * notas concatenadas. Para los campos de valor único se usa el de la primera
 * tarjeta; los descartados no se borran, se ven al separar (el banner lista los
 * miembros). Ver vasak-contacts#5 (decisión: fusión local primero).
 *
 * Un grupo fusionado se recuerda como las **claves** de sus miembros
 * (`ownCardKey`: la identidad de la tarjeta), no las tarjetas enteras, igual que
 * la libreta propia (`own-card.ts`).
 */

const MERGES_KEY = 'vasak-contacts.local-merges';

/** Deja el primero de cada clave, conservando el orden. */
function dedupBy<T>(items: T[], keyOf: (item: T) => string): T[] {
	const seen = new Set<string>();
	const out: T[] = [];
	for (const item of items) {
		const key = keyOf(item);
		if (seen.has(key)) continue;
		seen.add(key);
		out.push(item);
	}
	return out;
}

/**
 * Une un grupo de tarjetas en una sola, para mostrarla.
 *
 * La **primera** tarjeta manda en lo de valor único (nombre, organización, foto,
 * la identidad `url` con la que se la selecciona). Lo de muchos valores —correos,
 * teléfonos, direcciones, webs, redes, fechas, idiomas, categorías, relaciones,
 * campos a medida— se junta sin repetir, y las notas se concatenan. Nada se
 * pierde: lo que no entra en un campo de valor único sigue visible al separar.
 */
export function mergeContacts(members: Contact[]): Contact {
	const primary = members[0];
	const notes = dedupBy(
		members.map((m) => m.notes.trim()).filter((n) => n.length > 0),
		(n) => n
	).join('\n\n');
	return {
		...primary,
		emails: dedupBy(
			members.flatMap((m) => m.emails),
			(e) => e.value.trim().toLowerCase()
		),
		phones: dedupBy(
			members.flatMap((m) => m.phones),
			(p) => p.value.replace(/\D/g, '')
		),
		websites: dedupBy(
			members.flatMap((m) => m.websites),
			(w) => w.value.trim().toLowerCase()
		),
		addresses: dedupBy(
			members.flatMap((m) => m.addresses),
			(a) => JSON.stringify(a)
		),
		social: dedupBy(
			members.flatMap((m) => m.social),
			(s) => `${s.service}|${s.handle.trim().toLowerCase()}`
		),
		other_dates: dedupBy(
			members.flatMap((m) => m.other_dates),
			(d) => JSON.stringify(d)
		),
		languages: [...new Set(members.flatMap((m) => m.languages))],
		categories: [...new Set(members.flatMap((m) => m.categories))],
		related: dedupBy(
			members.flatMap((m) => m.related),
			(r) => `${r.kind}|${r.target_uid}|${r.name.trim().toLowerCase()}`
		),
		custom_fields: dedupBy(
			members.flatMap((m) => m.custom_fields),
			(c) => `${c.name}|${c.label}|${c.value}`
		),
		notes,
	};
}

/**
 * Aplica las fusiones locales a la lista: cada grupo con dos o más miembros
 * presentes se reemplaza por la tarjeta unida, en el lugar del primero. Un grupo
 * del que ya no queda más que un miembro no se fusiona (el otro se borró o cambió
 * de cuenta): se muestra tal cual.
 *
 * Devuelve la lista nueva y, por la clave de cada tarjeta unida, sus miembros
 * —para el botón de separar y para ver qué se juntó.
 */
export function applyLocalMerges(
	contacts: Contact[],
	groups: string[][]
): { contacts: Contact[]; members: Map<string, Contact[]> } {
	const byKey = new Map<string, Contact>();
	for (const contact of contacts) {
		const key = ownCardKey(contact);
		if (key && !byKey.has(key)) byKey.set(key, contact);
	}

	// La tarjeta unida de cada grupo viable, y a qué grupo pertenece cada clave
	// (gana el primer grupo, por si una clave aparece en dos).
	const mergedByGroup = new Map<number, Contact>();
	const members = new Map<string, Contact[]>();
	const groupOfKey = new Map<string, number>();
	groups.forEach((group, index) => {
		const present = group.map((k) => byKey.get(k)).filter((c): c is Contact => c !== undefined);
		if (present.length < 2) return;
		for (const member of present) {
			const key = ownCardKey(member);
			if (!groupOfKey.has(key)) groupOfKey.set(key, index);
		}
		const merged = mergeContacts(present);
		mergedByGroup.set(index, merged);
		members.set(ownCardKey(merged), present);
	});

	const out: Contact[] = [];
	const emitted = new Set<number>();
	for (const contact of contacts) {
		const index = groupOfKey.get(ownCardKey(contact));
		if (index === undefined) {
			out.push(contact);
			continue;
		}
		if (!emitted.has(index)) {
			emitted.add(index);
			const merged = mergedByGroup.get(index);
			if (merged) out.push(merged);
		}
	}
	return { contacts: out, members };
}

/** Lee las fusiones locales guardadas. Si no se puede, no hay ninguna. */
export function loadMerges(): string[][] {
	try {
		const raw = globalThis.localStorage?.getItem(MERGES_KEY);
		if (!raw) return [];
		const parsed: unknown = JSON.parse(raw);
		if (!Array.isArray(parsed)) return [];
		return parsed
			.filter((group): group is unknown[] => Array.isArray(group))
			.map((group) => group.filter((k): k is string => typeof k === 'string'))
			.filter((group) => group.length >= 2);
	} catch {
		return [];
	}
}

/** Guarda las fusiones locales. Si no se puede, valen para esta sesión igual. */
export function saveMerges(groups: string[][]): void {
	try {
		globalThis.localStorage?.setItem(MERGES_KEY, JSON.stringify(groups));
	} catch {
		// No recordar la fusión no rompe lo que se está viendo.
	}
}
