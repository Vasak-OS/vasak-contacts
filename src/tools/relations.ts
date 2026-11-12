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
	/**
	 * `forward`: la declara este contacto. `inverse`: otro lo nombra a él y se
	 * calcula para mostrarla desde su lado («jefe/a de Ana»).
	 */
	direction: 'forward' | 'inverse';
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
			direction: 'forward' as const,
		};
	});
}

/**
 * Las relaciones que OTROS contactos declaran hacia este, calculadas para
 * mostrarlas desde su lado: si A dice que su jefe es B, en la ficha de B aparece
 * «jefe/a de A». No están en la tarjeta de B; salen de recorrer las demás.
 *
 * Sólo las que apuntan por `target_uid` (las de texto no se pueden atribuir a
 * una ficha concreta sin afirmar que es esa persona).
 */
export function inverseRelated(contact: Contact, allContacts: Contact[]): ResolvedRelation[] {
	const self = normalizeUid(contact.uid);
	if (!self) {
		return [];
	}
	const out: ResolvedRelation[] = [];
	for (const other of allContacts) {
		if (normalizeUid(other.uid) === self) {
			continue;
		}
		for (const relation of other.related) {
			if (relation.target_uid && normalizeUid(relation.target_uid) === self) {
				out.push({
					kind: relation.kind,
					name: other.name,
					contact: other,
					direction: 'inverse',
				});
			}
		}
	}
	return out;
}

/**
 * Las relaciones a mostrar en la ficha: las propias y las que otros declaran
 * hacia ella, sin repetir a alguien que ya aparece de los dos lados —si B dice
 * que A es su pareja, la pareja de A hacia B no se muestra dos veces.
 */
export function relationsFor(contact: Contact, allContacts: Contact[]): ResolvedRelation[] {
	const forward = resolveRelated(contact, allContacts);
	const yaMostrados = new Set(
		forward
			.map((relation) => (relation.contact ? normalizeUid(relation.contact.uid) : ''))
			.filter((uid) => uid !== '')
	);
	const inverse = inverseRelated(contact, allContacts).filter(
		(relation) => !(relation.contact && yaMostrados.has(normalizeUid(relation.contact.uid)))
	);
	return [...forward, ...inverse];
}
