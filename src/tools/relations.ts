import type { Contact } from './address-book';

/**
 * Una relación lista para mostrar: el tipo, el nombre y —si se pudo resolver—
 * la otra ficha, para enlazarla.
 */
export interface ResolvedRelation {
	kind: string;
	/** El nombre a mostrar: el de la ficha resuelta, o el que traía la relación. */
	name: string;
	/** La otra ficha, cuando el `target_uid` resolvió a un contacto cargado. */
	contact: Contact | null;
}

const URN_UUID = /^urn:uuid:/i;

/**
 * El uid sin el esquema `urn:uuid:`, para comparar.
 *
 * El `uid` de un contacto llega crudo del backend —puede venir como
 * `urn:uuid:<x>` o pelado—, y el `target_uid` de una relación viene pelado. Hay
 * que quitarle el prefijo a **los dos** antes de compararlos, o una tarjeta con
 * el uid en forma URN no engancharía con la relación que la nombra.
 */
export function normalizeUid(uid: string): string {
	return uid.replace(URN_UUID, '').trim();
}

/**
 * Las relaciones del contacto, con la otra ficha resuelta cuando se puede.
 *
 * Una relación por `target_uid` se busca entre los contactos cargados por uid
 * normalizado. Ante dos tarjetas con el mismo uid gana la primera, de forma
 * determinista: no se inventa cuál. Una relación a sí mismo no se enlaza.
 */
export function resolveRelated(contact: Contact, allContacts: Contact[]): ResolvedRelation[] {
	const byUid = new Map<string, Contact>();
	for (const candidate of allContacts) {
		const key = normalizeUid(candidate.uid);
		if (key && !byUid.has(key)) {
			byUid.set(key, candidate);
		}
	}
	const self = normalizeUid(contact.uid);
	return contact.related.map((relation) => {
		const key = normalizeUid(relation.target_uid);
		const other = key && key !== self ? (byUid.get(key) ?? null) : null;
		return {
			kind: relation.kind,
			name: other?.name || relation.name,
			contact: other,
		};
	});
}
