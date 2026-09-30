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
 * Los contactos que coinciden con lo que se escribió.
 *
 * Se busca en el nombre, el apodo, la organización, el cargo, los correos y los
 * teléfonos:
 * la gente busca por lo que recuerda, y lo que recuerda a veces es «el que
 * trabaja en Vasak» o los últimos cuatro dígitos de un número.
 *
 * Cada palabra tiene que aparecer en **alguna** parte, no todas en la misma:
 * escribir «ana vasak» encuentra a Ana que trabaja en Vasak, que es lo que
 * quiere decir quien lo escribe.
 */
export function searchContacts(contacts: Contact[], query: string): Contact[] {
	const words = withoutAccents(query)
		.split(/\s+/)
		.filter((p) => p.length > 0);
	if (words.length === 0) {
		return contacts;
	}

	return contacts.filter((contact) => {
		const haystack = withoutAccents(
			[
				contact.name,
				// El apodo es como muchos buscan a alguien: «Pato» y no «Joaquín».
				contact.nickname,
				contact.organization,
				contact.title,
				...contact.emails.map((c) => c.value),
				// Los teléfonos, **sólo sus dígitos**: nadie busca «11-5555»
				// escribiendo el guión en el mismo lugar, y los separadores que
				// usa la gente no son una lista corta — enumerar el guión, el
				// punto y el paréntesis dejaba afuera «11/5555-1234».
				...contact.phones.map((t) => t.value.replace(/\D/g, '')),
			].join(' ')
		);

		return words.every((word) => {
			if (haystack.includes(word)) {
				return true;
			}
			// Y si la palabra tiene dígitos, también por sus dígitos solos: así
			// «(11) 5555-1234» encuentra un número guardado como «11/5555 1234».
			//
			// **Sólo si quedan dígitos.** Una palabra sin ninguno queda en la
			// cadena vacía, y todo texto la contiene: sin este control, buscar
			// «nadie» devolvía la agenda entera.
			const digitsOnly = word.replace(/\D/g, '');
			return digitsOnly.length > 0 && haystack.includes(digitsOnly);
		});
	});
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
