/**
 * Ordenar y buscar en la agenda.
 *
 * Aparte de los componentes y sin nada de Vue adentro. Es lo único de una
 * libreta que se equivoca **en silencio**: un orden que manda a los Álvarez al
 * final se ve prolijo, y una búsqueda que no encuentra a «Perez» cuando el
 * contacto es «Pérez» simplemente dice que no hay nadie.
 */

/** Un dato con etiqueta: un correo, un teléfono, una web. */
export interface LabeledValue {
	/** «casa», «trabajo», «celular»… o vacío si la tarjeta no dijo. */
	label: string;
	value: string;
}

/** Una dirección postal, con sus siete partes. Ver `addressLines`. */
export interface Address {
	label: string;
	po_box: string;
	extended: string;
	street: string;
	locality: string;
	region: string;
	postal_code: string;
	country: string;
}

/**
 * Una fecha que puede venir a medias: sin año es lo más común en un
 * cumpleaños. Si no se pudo leer, las partes van en `null` y `text` trae lo
 * que decía la tarjeta.
 */
export interface PartialDate {
	year: number | null;
	month: number | null;
	day: number | null;
	text: string;
}

export interface LabeledDate {
	label: string;
	date: PartialDate;
}

/** Una cuenta en una red o en un servicio de mensajería. */
export interface SocialProfile {
	service: string;
	label: string;
	handle: string;
	/** Sólo `http` o `https`; vacía si no hay una dirección que abrir. */
	url: string;
}

/** Un `X-` que no es de nadie: un «campo a medida». */
export interface CustomField {
	name: string;
	label: string;
	value: string;
}

/** Por qué una tarjeta con foto se muestra con las iniciales. */
export type PhotoSkipped = 'too_large' | 'unsupported' | 'insecure';

/**
 * El estado de sincronización de un contacto, como lo entrega el almacén: al
 * día, un cambio local sin subir, o un cambio que chocó con el servidor. La
 * misma forma que `SyncState` de `vasak-accounts` (texto en minúscula).
 */
export type SyncState = 'synced' | 'pending' | 'conflict';

/** Un contacto tal como llega del programa. */
export interface Contact {
	uid: string;
	name: string;
	/** «Pérez, Ana»: con qué ordenarlo. Ver `sortContacts`. */
	sort_key: string;
	emails: LabeledValue[];
	phones: LabeledValue[];
	organization: string;
	notes: string;
	url: string;
	/**
	 * En qué libreta vive la tarjeta: el id de la libreta del almacén, o su
	 * dirección cuando la lectura va al servidor. Lo pone el programa, no sale
	 * de la tarjeta sino de **dónde estaba**. Sirve para elegir qué libretas se
	 * ven (`tools/library-filter.ts`).
	 */
	address_book_id: string;
	/**
	 * El número local del contacto en el almacén, para editarlo o borrarlo. Lo
	 * pone el programa (`store::contacts_from`) con el id de la tarjeta; vacío
	 * cuando la lectura va al servidor o es una sub-tarjeta de un recurso con
	 * varias. Es la fila de la base, no el `uid` de la vCard.
	 */
	store_id: string;
	/**
	 * El ETag de la tarjeta: la precondición para editarla sin pisar un cambio
	 * de otro lado. Lo pone el programa con lo que entrega el almacén; vacío
	 * cuando la lectura va al servidor, cuando es una creación local sin
	 * confirmar, o cuando es una sub-tarjeta de un recurso con varias. Mientras
	 * viaje vacío, la ventana deja «editar» pendiente (ver `canEdit`).
	 */
	etag: string;
	/**
	 * El estado de sincronización de la tarjeta, para la insignia «sin
	 * sincronizar» y para saber si hay un conflicto que resolver. Lo pone el
	 * programa con lo que entrega el almacén; `synced` cuando la lectura va al
	 * servidor o es una sub-tarjeta.
	 */
	sync_state: SyncState;
	/**
	 * De qué cuenta es el contacto. **Lo pone la ventana** al juntar lo de cada
	 * cuenta (`use-agenda.ts`), no el programa: dos cuentas pueden tener una
	 * libreta con el mismo id, y sin la cuenta no se distinguen al filtrar.
	 */
	account_id: string;
	/** La foto como `data:` URI; vacía si no vino adentro de la tarjeta. */
	photo: string;
	/**
	 * La dirección de una foto que está afuera de la tarjeta. **La ventana no
	 * la pide nunca**: se la pide al programa, que la baja una vez y la
	 * guarda. Ver `photo-loader.ts`.
	 */
	photo_url: string;
	photo_skipped: PhotoSkipped | null;
	nickname: string;
	title: string;
	role: string;
	kind: string;
	addresses: Address[];
	birthday: PartialDate | null;
	anniversary: PartialDate | null;
	other_dates: LabeledDate[];
	websites: LabeledValue[];
	social: SocialProfile[];
	languages: string[];
	time_zone: string;
	geo: string;
	categories: string[];
	custom_fields: CustomField[];
}

/**
 * Un contacto vacío con lo que se le pase encima.
 *
 * Para las pruebas: los campos son muchos, y armar uno a mano con todos es
 * donde se olvida uno.
 */
export function emptyContact(overrides: Partial<Contact> = {}): Contact {
	return {
		uid: '',
		name: '',
		sort_key: '',
		emails: [],
		phones: [],
		organization: '',
		notes: '',
		url: '',
		address_book_id: '',
		store_id: '',
		etag: '',
		sync_state: 'synced',
		account_id: '',
		photo: '',
		photo_url: '',
		photo_skipped: null,
		nickname: '',
		title: '',
		role: '',
		kind: '',
		addresses: [],
		birthday: null,
		anniversary: null,
		other_dates: [],
		websites: [],
		social: [],
		languages: [],
		time_zone: '',
		geo: '',
		categories: [],
		custom_fields: [],
		...overrides,
	};
}

/**
 * Si un contacto se puede borrar: hace falta su número local en el almacén. Lo
 * que vino del servidor, o una sub-tarjeta de un recurso con varias, no lo
 * tiene, y no hay fila que borrar.
 */
export function canDelete(contact: Contact): boolean {
	return contact.store_id.trim().length > 0;
}

/**
 * Si un contacto se puede editar: hace falta su número local **y** su ETag.
 *
 * Sin ETag, editar pisaría a ciegas un cambio hecho desde otro lado, así que la
 * ventana lo deja «pendiente» hasta que el almacén entregue el ETag en los DTOs
 * (vasak-contacts#2, punto 5). Un contacto recién creado tampoco lo tiene hasta
 * que sincroniza, así que no se puede editar enseguida: hay que esperar la
 * vuelta.
 */
export function canEdit(contact: Contact): boolean {
	return contact.store_id.trim().length > 0 && contact.etag.trim().length > 0;
}

/**
 * Ordena la agenda por apellido, en el idioma de la sesión.
 *
 * **Con `Intl.Collator` y no comparando cadenas.** Comparar por código de
 * carácter manda «Álvarez» después de «Zaparte», porque la Á está por encima de
 * cualquier letra ASCII: en español eso desordena media agenda. El comparador
 * del sistema sabe que la Á va con la A, que la Ñ va después de la N, y lo
 * mismo para los idiomas que nosotros no conocemos.
 *
 * `sensitivity: 'base'` hace además que las mayúsculas no separen a los Pérez
 * de los pérez, que es lo que se espera de una lista de nombres.
 */
export function sortContacts(contacts: Contact[], locale: string): Contact[] {
	const collator = new Intl.Collator(locale, { sensitivity: 'base', numeric: true });
	// Sobre una copia: ordenar el arreglo original mutaría lo que vino del
	// programa, y en Vue eso dispara cada dependencia que lo esté mirando.
	return [...contacts].sort((a, b) => collator.compare(a.sort_key, b.sort_key));
}

/**
 * Quita los acentos para comparar.
 *
 * Buscar «Perez» tiene que encontrar a «Pérez». Nadie escribe los acentos en un
 * buscador —ni siquiera quien los escribe bien en todo lo demás—, y una
 * búsqueda que no los ignora simplemente dice que no hay nadie.
 *
 * `NFD` separa la letra de su tilde y el rango que se saca son las tildes: es
 * lo que convierte «é» en «e» sin tener que enumerar las letras del alfabeto de
 * ningún idioma.
 */
export function withoutAccents(text: string): string {
	return text.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
}

/**
 * Dónde coincidió una palabra. **Sólo el nombre del campo, nunca su texto**:
 * las notas son lo más privado de la ficha, y una razón como «coincide en
 * notas» está bien mientras que mostrar el fragmento no. Quien dibuje la razón
 * parte de acá y no tiene de dónde sacar el contenido.
 */
export type SearchField =
	| 'name'
	| 'nickname'
	| 'organization'
	| 'title'
	| 'email'
	| 'phone'
	| 'address'
	| 'notes'
	| 'categories';

/** Un contacto encontrado, con qué tan bien coincide y en qué campos. */
export interface ContactMatch {
	contact: Contact;
	/** Mayor es mejor: suma, por palabra, el mejor tipo de coincidencia. */
	score: number;
	fields: SearchField[];
}

/** Distancia de edición entre dos cadenas (Levenshtein, dos filas). */
export function levenshtein(a: string, b: string): number {
	let prev = Array.from({ length: b.length + 1 }, (_, j) => j);
	for (let i = 1; i <= a.length; i++) {
		const row = [i];
		for (let j = 1; j <= b.length; j++) {
			row[j] = Math.min(prev[j] + 1, row[j - 1] + 1, prev[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1));
		}
		prev = row;
	}
	return prev[b.length];
}

/**
 * La forma «de oído» de una palabra en español, para comparar cómo suena y no
 * cómo se escribe: `s/z/c` suenan igual (`c` sólo ante e/i), `b/v`, `y/ll`,
 * `g/j` (`g` sólo ante e/i), `qu/k` y la `h` es muda.
 *
 * **No es Soundex ni Metaphone**: están pensados para el inglés y en español
 * dan uniones absurdas. Estas reglas son cortas, se prueban una por una y no
 * suman dependencia. Espera la palabra ya sin acentos y en minúsculas
 * (`withoutAccents`).
 */
export function phoneticKey(word: string): string {
	return (
		word
			.replace(/ll/g, 'y')
			.replace(/ch/g, 'x')
			.replace(/h/g, '')
			.replace(/qu/g, 'k')
			// «gue/gui» suena /ge/ con g dura: se marca para que la regla de g→j
			// de abajo no la toque.
			.replace(/gu(?=[ei])/g, 'G')
			.replace(/g(?=[ei])/g, 'j')
			.replace(/c(?=[ei])/g, 's')
			.replace(/[zs]/g, 's')
			.replace(/[ck]/g, 'k')
			.replace(/v/g, 'b')
			.replace(/G/g, 'g')
	);
}

/** Cuántas ediciones se toleran, según el largo: en una palabra corta una es demasiado. */
function typoTolerance(length: number): number {
	if (length <= 3) return 0;
	return length <= 6 ? 1 : 2;
}

const MATCH_EXACT = 4;
const MATCH_PREFIX = 3;
const MATCH_SUBSTRING = 2;
const MATCH_PHONETIC = 1;

interface IndexedField {
	field: SearchField;
	text: string;
	tokens: string[];
	/** Sólo dígitos: se compara contra los dígitos de lo que se escribió. */
	isPhone: boolean;
}

function indexContact(contact: Contact): IndexedField[] {
	const raw: [SearchField, string][] = [
		['name', contact.name],
		// El apodo es como muchos buscan a alguien: «Pato» y no «Joaquín».
		['nickname', contact.nickname],
		['organization', contact.organization],
		['title', contact.title],
		['title', contact.role],
		...contact.emails.map((c): [SearchField, string] => ['email', c.value]),
		// Los teléfonos, **sólo sus dígitos**: nadie busca «11-5555»
		// escribiendo el guión en el mismo lugar, y los separadores que usa la
		// gente no son una lista corta — enumerar el guión, el punto y el
		// paréntesis dejaba afuera «11/5555-1234».
		...contact.phones.map((t): [SearchField, string] => ['phone', t.value.replace(/\D/g, '')]),
		...contact.addresses.map((d): [SearchField, string] => [
			'address',
			[d.po_box, d.extended, d.street, d.locality, d.region, d.postal_code, d.country].join(' '),
		]),
		['notes', contact.notes],
		...contact.categories.map((c): [SearchField, string] => ['categories', c]),
	];
	return raw
		.filter(([, text]) => text.trim().length > 0)
		.map(([field, value]) => {
			const text = withoutAccents(value);
			return {
				field,
				text,
				tokens: text.split(/[^\p{L}\p{N}]+/u).filter((t) => t.length > 0),
				isPhone: field === 'phone',
			};
		});
}

/** Cómo coincide una palabra con un campo: 0 si no coincide. */
function scoreWord(word: string, f: IndexedField): number {
	if (f.text.includes(word)) {
		if (f.tokens.includes(word)) return MATCH_EXACT;
		return f.tokens.some((t) => t.startsWith(word)) ? MATCH_PREFIX : MATCH_SUBSTRING;
	}
	// Y si la palabra tiene dígitos, también por sus dígitos solos: así
	// «(11) 5555-1234» encuentra un número guardado como «11/5555 1234».
	//
	// **Sólo si quedan dígitos.** Una palabra sin ninguno queda en la cadena
	// vacía, y todo texto la contiene: sin este control, buscar «nadie»
	// devolvía la agenda entera.
	const digitsOnly = word.replace(/\D/g, '');
	if (digitsOnly.length > 0) {
		return f.isPhone && f.text.includes(digitsOnly) ? MATCH_SUBSTRING : 0;
	}
	if (f.isPhone) return 0;

	const key = phoneticKey(word);
	const tolerance = typoTolerance(key.length);
	return f.tokens.some((t) => {
		const tokenKey = phoneticKey(t);
		return (
			tokenKey === key ||
			(tolerance > 0 &&
				Math.abs(tokenKey.length - key.length) <= tolerance &&
				levenshtein(tokenKey, key) <= tolerance)
		);
	})
		? MATCH_PHONETIC
		: 0;
}

/**
 * Los contactos que coinciden con lo que se escribió, del que mejor coincide al
 * que peor: exacta > prefijo > subcadena > fonética. A igual calidad se
 * conserva el orden de entrada (el del idioma, ver `sortContacts`).
 *
 * Se busca en el nombre, el apodo, la organización, el cargo, los correos, los
 * teléfonos, las direcciones, las notas y las etiquetas: la gente busca por lo
 * que recuerda, y lo que recuerda a veces es «el que trabaja en Vasak» o los
 * últimos cuatro dígitos de un número. Y se tolera el error de oído o de
 * tipeo: «Gonsález» encuentra a «Gonzalez».
 *
 * Cada palabra tiene que aparecer en **alguna** parte, no todas en la misma:
 * escribir «ana vasak» encuentra a Ana que trabaja en Vasak, que es lo que
 * quiere decir quien lo escribe.
 */
export function searchMatches(contacts: Contact[], query: string): ContactMatch[] {
	const words = withoutAccents(query)
		.split(/\s+/)
		.filter((p) => p.length > 0);
	if (words.length === 0) {
		return contacts.map((contact) => ({ contact, score: 0, fields: [] }));
	}

	const found: ContactMatch[] = [];
	for (const contact of contacts) {
		const indexed = indexContact(contact);
		const fields = new Set<SearchField>();
		let score = 0;
		const allFound = words.every((word) => {
			let best = 0;
			let bestField: SearchField | null = null;
			for (const f of indexed) {
				const s = scoreWord(word, f);
				if (s > best) {
					best = s;
					bestField = f.field;
				}
			}
			if (bestField === null) return false;
			fields.add(bestField);
			score += best;
			return true;
		});
		if (allFound) found.push({ contact, score, fields: [...fields] });
	}
	// `sort` es estable: a igual puntaje queda el orden con que llegaron.
	return found.sort((a, b) => b.score - a.score);
}

/** Los contactos que coinciden con lo que se escribió. Ver `searchMatches`. */
export function searchContacts(contacts: Contact[], query: string): Contact[] {
	return searchMatches(contacts, query).map((m) => m.contact);
}

/**
 * La inicial con la que se agrupa un contacto.
 *
 * Sin acento, para que Álvarez y Alvarez caigan en la misma letra en vez de en
 * dos grupos separados por todo el alfabeto. Lo que no empieza con una letra
 * —un nombre de empresa que arranca con un número, un contacto sin nombre— va a
 * un grupo aparte, porque mezclarlo con la A es peor que darle el suyo.
 */
export function initialOf(contact: Contact): string {
	const first = withoutAccents(contact.sort_key.trim()).charAt(0).toUpperCase();
	return /[A-Z]/.test(first) ? first : '#';
}
