import { type Contact, withoutAccents } from './address-book';
import { normalizeUid } from './relations';

/**
 * Un grupo de tarjetas que parecen la misma persona. **Una propuesta**, no una
 * fusión: quien usa el sistema confirma. Nunca se fusiona solo.
 */
export interface DuplicateGroup {
	contacts: Contact[];
}

/**
 * Cuántos dígitos del final se comparan de un teléfono. Comparar por subcadena
 * une a dos personas distintas —`551234` es subcadena de `541155551234`—, así
 * que se comparan los últimos dígitos, y sólo cuando hay suficientes: un número
 * con menos que esto trae muy poca información para arriesgar una unión.
 *
 * shortcut: largo fijo pensado para el número local de acá; un plan de
 * numeración muy corto emparejaría de más. Upgrade si molesta: longitud por país.
 */
const PHONE_SIGNIFICANT = 7;

/** Las claves de teléfono de un contacto: los últimos dígitos de cada número con suficientes. */
function phoneKeys(contact: Contact): string[] {
	const keys: string[] = [];
	for (const phone of contact.phones) {
		const digits = phone.value.replace(/\D/g, '');
		if (digits.length >= PHONE_SIGNIFICANT) keys.push(digits.slice(-PHONE_SIGNIFICANT));
	}
	return keys;
}

/** Las claves de correo de un contacto: cada dirección en minúsculas, sin espacios. */
function emailKeys(contact: Contact): string[] {
	return contact.emails
		.map((email) => email.value.trim().toLowerCase())
		.filter((value) => value.length > 0);
}

/** La clave de nombre: sin acentos ni mayúsculas, o vacía si no tiene nombre. */
function nameKey(contact: Contact): string {
	return withoutAccents(contact.name).trim();
}

/** Union-find sobre los índices del arreglo, para juntar en grupos sin comparar todos contra todos de más. */
class UnionFind {
	private parent: number[];
	constructor(size: number) {
		this.parent = Array.from({ length: size }, (_, i) => i);
	}
	find(i: number): number {
		while (this.parent[i] !== i) {
			this.parent[i] = this.parent[this.parent[i]];
			i = this.parent[i];
		}
		return i;
	}
	union(a: number, b: number): void {
		const ra = this.find(a);
		const rb = this.find(b);
		if (ra !== rb) this.parent[ra] = rb;
	}
}

/** Junta en el union-find los índices que comparten una misma clave (un mismo bucket). */
function unionBySharedKey(uf: UnionFind, keysOf: (i: number) => string[], size: number): void {
	const buckets = new Map<string, number>();
	for (let i = 0; i < size; i++) {
		for (const key of keysOf(i)) {
			const first = buckets.get(key);
			if (first === undefined) buckets.set(key, i);
			else uf.union(first, i);
		}
	}
}

/** La clave ordenada de un par de índices, para contar señales por par sin repetir. */
function pairKey(a: number, b: number): string {
	return a < b ? `${a}-${b}` : `${b}-${a}`;
}

/**
 * Los grupos de tarjetas que parecen la misma persona, para proponer una fusión.
 *
 * Las señales, de más fuerte a más débil:
 * - **Mismo UID** (normalizado): es la misma tarjeta sincronizada en dos
 *   libretas. Alcanza sola.
 * - Las débiles —**mismo correo**, **mismo teléfono** (por los últimos dígitos,
 *   no por subcadena) y **mismo nombre**— exigen **dos** para proponer: una sola
 *   da demasiados falsos positivos, y **nunca se propone sólo por el nombre**
 *   (dos «Juan Pérez» son casi siempre dos personas).
 *
 * Se juntan por componente conexo: si A y B comparten dos señales y B y C otras
 * dos, los tres quedan en el mismo grupo para que la persona los revise junta.
 * El orden del arreglo se conserva dentro de cada grupo.
 */
export function findDuplicates(contacts: Contact[]): DuplicateGroup[] {
	const n = contacts.length;
	const uf = new UnionFind(n);

	// Fuerte: el mismo UID junta directo.
	unionBySharedKey(
		uf,
		(i) => {
			const key = normalizeUid(contacts[i].uid);
			return key ? [key] : [];
		},
		n
	);

	// Débiles: se cuentan los TIPOS de señal que comparte cada par (un par con dos
	// correos iguales comparte un solo tipo), y se junta al llegar a dos.
	const types: ((i: number) => string[])[] = [
		(i) => emailKeys(contacts[i]),
		(i) => phoneKeys(contacts[i]),
		(i) => {
			const key = nameKey(contacts[i]);
			return key ? [key] : [];
		},
	];
	const sharedTypes = new Map<string, number>();
	for (const keysOf of types) {
		const buckets = new Map<string, number[]>();
		for (let i = 0; i < n; i++) {
			for (const key of keysOf(i)) {
				const bucket = buckets.get(key) ?? [];
				bucket.push(i);
				buckets.set(key, bucket);
			}
		}
		// Un tipo cuenta una vez por par, aunque compartan dos claves del mismo tipo.
		const countedThisType = new Set<string>();
		for (const bucket of buckets.values()) {
			for (let x = 0; x < bucket.length; x++) {
				for (let y = x + 1; y < bucket.length; y++) {
					const pk = pairKey(bucket[x], bucket[y]);
					if (countedThisType.has(pk)) continue;
					countedThisType.add(pk);
					const count = (sharedTypes.get(pk) ?? 0) + 1;
					sharedTypes.set(pk, count);
					if (count >= 2) {
						uf.union(bucket[x], bucket[y]);
					}
				}
			}
		}
	}

	// Armar los grupos por raíz, conservando el orden de entrada.
	const byRoot = new Map<number, Contact[]>();
	for (let i = 0; i < n; i++) {
		const root = uf.find(i);
		const group = byRoot.get(root) ?? [];
		group.push(contacts[i]);
		byRoot.set(root, group);
	}
	return [...byRoot.values()]
		.filter((group) => group.length >= 2)
		.map((contacts) => ({ contacts }));
}

/** Cuántas tarjetas caen en algún grupo de posibles repetidos. */
export function duplicateCount(groups: DuplicateGroup[]): number {
	return groups.reduce((total, group) => total + group.contacts.length, 0);
}
