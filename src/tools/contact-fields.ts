/**
 * Cómo se muestra cada campo de la ficha.
 *
 * Aparte del componente y sin nada de Vue adentro, para poder probarlo sin la
 * ventana: son las reglas que se equivocan en silencio —una fecha sin año que
 * sale «15 de abril de 1970», una web `javascript:` que se abre, una dirección
 * con renglones vacíos—.
 */

import type { Address, CustomField, PartialDate } from '@/tools/address-book';

/**
 * Los renglones de una dirección, sin los vacíos.
 *
 * Calle, ciudad, provincia, código y país, cada uno en su renglón. Cómo se
 * escribe una dirección de verdad depende del país —en unos el código va antes
 * de la ciudad, en otros después, en otros en el renglón de la provincia— y
 * hacerlo bien pide una tabla por país que no tenemos. Un renglón por parte es
 * lo que se lee en cualquier país sin equivocarse.
 *
 * El apartado de correos va primero y el «piso, depto» después de la calle,
 * que es donde lo busca cualquiera.
 */
export function addressLines(address: Address): string[] {
	return [
		address.po_box,
		address.street,
		address.extended,
		address.locality,
		address.region,
		address.postal_code,
		address.country,
	]
		.map((part) => part.trim())
		.filter((part) => part.length > 0);
}

/**
 * Una fecha a medias, en el idioma de la sesión.
 *
 * **Sin año no es un error**: es alguien que no quiere decirlo. Se muestra el
 * día y el mes y nada más, en vez de inventar un año —que es lo que pasa con
 * `new Date(mes, día)`— o de no mostrar nada.
 *
 * En UTC, porque la fecha no tiene hora: armarla en la zona de la sesión la
 * corre un día al oeste de Greenwich.
 */
export function formatPartialDate(date: PartialDate, locale: string): string {
	if (date.text) {
		return date.text;
	}
	const { year, month, day } = date;
	// 2000 fue bisiesto: sin año, el 29 de febrero tiene que existir.
	const at = new Date(Date.UTC(year ?? 2000, (month ?? 1) - 1, day ?? 1));
	const format = (options: Intl.DateTimeFormatOptions) =>
		new Intl.DateTimeFormat(locale, { ...options, timeZone: 'UTC' }).format(at);

	if (year !== null && month !== null && day !== null) {
		return format({ year: 'numeric', month: 'long', day: 'numeric' });
	}
	if (month !== null && day !== null) {
		return format({ month: 'long', day: 'numeric' });
	}
	if (year !== null && month !== null) {
		return format({ year: 'numeric', month: 'long' });
	}
	if (month !== null) {
		return format({ month: 'long' });
	}
	if (year !== null) {
		return String(year);
	}
	if (day !== null) {
		return String(day);
	}
	return '';
}

/**
 * La dirección que se abre al tocar una web, o vacío si no se debe abrir.
 *
 * **Sólo `http` y `https`.** La tarjeta la escribió cualquiera, y una «web»
 * `javascript:`, `file:` o `smb:` abierta con el sistema hace otra cosa que
 * mostrar una página. Lo que no trae esquema —`ana.ejemplo.com`, que es como lo
 * escribe mucha gente— se abre por `https`.
 *
 * Una dirección con usuario adentro (`https://ana@ejemplo.com`) tampoco: es
 * cómo se disfraza un sitio de otro, y además un correo mal puesto en `URL` se
 * vería así.
 */
export function webLink(value: string): string {
	const trimmed = value.trim();
	// biome-ignore lint/suspicious/noControlCharactersInRegex: justamente se buscan los controles.
	if (!trimmed || /[\s\u0000-\u001f\u007f]/.test(trimmed)) {
		return '';
	}
	const hasScheme = /^[a-z][a-z0-9+.-]*:/i.test(trimmed);
	const candidate = hasScheme ? trimmed : `https://${trimmed}`;
	try {
		const url = new URL(candidate);
		if (url.protocol !== 'https:' && url.protocol !== 'http:') {
			return '';
		}
		if (!url.hostname || url.username || url.password) {
			return '';
		}
		return url.href;
	} catch {
		return '';
	}
}

/**
 * Las iniciales de un nombre, para cuando no hay foto: la primera letra de la
 * primera y de la última palabra.
 *
 * Por caracteres y no por unidades de UTF-16: con `charAt` un nombre que
 * empieza con un carácter fuera del plano básico dejaba medio carácter, que
 * se dibuja como un rombo.
 */
export function initialsOf(name: string): string {
	const words = name
		.trim()
		.split(/\s+/)
		.filter((w) => w.length > 0)
		// Lo que no empieza con una letra —«(Trabajo)», «3M»— no da una inicial
		// que se entienda.
		.filter((w) => /^\p{L}/u.test(w));
	if (words.length === 0) {
		return '';
	}
	const first = Array.from(words[0])[0] ?? '';
	const last = words.length > 1 ? (Array.from(words[words.length - 1])[0] ?? '') : '';
	return (first + last).toLocaleUpperCase();
}

/**
 * El nombre de un idioma en el idioma de la sesión: `es-AR` es «español
 * (Argentina)» para quien lee en español y «Spanish (Argentina)» para quien
 * lee en inglés. El sistema ya sabe hacerlo; la etiqueta cruda si no.
 */
export function languageName(tag: string, locale: string): string {
	try {
		return new Intl.DisplayNames([locale], { type: 'language' }).of(tag) ?? tag;
	} catch {
		return tag;
	}
}

/**
 * El nombre que se muestra de un campo a medida.
 *
 * La etiqueta, si la tiene —es lo que escribió la persona—, y si no el nombre
 * de la propiedad sin el `X-` y con espacios: `X-PHONETIC-FIRST-NAME` se lee
 * «Phonetic first name». Traducirlo no se puede: el nombre lo inventó quien
 * exportó la tarjeta.
 */
export function customFieldName(field: CustomField): string {
	if (field.label.trim()) {
		return field.label.trim();
	}
	const bare = field.name.replace(/^x-/i, '').replace(/[-_]+/g, ' ').trim().toLowerCase();
	return bare ? bare.charAt(0).toUpperCase() + bare.slice(1) : field.name;
}

/**
 * Las etiquetas de siempre, que se traducen. Las que escribió la persona
 * («casa de la playa») se muestran como vinieron.
 */
const KNOWN_LABELS = new Set([
	'home',
	'work',
	'cell',
	'mobile',
	'main',
	'fax',
	'pager',
	'other',
	'homepage',
	'blog',
	'profile',
	'anniversary',
	'school',
]);

/** La clave de traducción de una etiqueta conocida, o `null`. */
export function labelKey(label: string): string | null {
	const normalized = label.trim().toLowerCase();
	// `iPhone` es una etiqueta de Apple que quiere decir «celular».
	if (normalized === 'iphone') {
		return 'contact.labels.cell';
	}
	return KNOWN_LABELS.has(normalized) ? `contact.labels.${normalized}` : null;
}

/**
 * Los nombres propios de los servicios. Son marcas: no se traducen, pero
 * tampoco se muestran en minúsculas como vienen de la tarjeta.
 */
const SERVICE_NAMES: Record<string, string> = {
	aim: 'AIM',
	facebook: 'Facebook',
	flickr: 'Flickr',
	gadugadu: 'Gadu-Gadu',
	github: 'GitHub',
	gtalk: 'Google Talk',
	icq: 'ICQ',
	instagram: 'Instagram',
	irc: 'IRC',
	linkedin: 'LinkedIn',
	mastodon: 'Mastodon',
	matrix: 'Matrix',
	msn: 'MSN',
	myspace: 'Myspace',
	qq: 'QQ',
	signal: 'Signal',
	sip: 'SIP',
	skype: 'Skype',
	telegram: 'Telegram',
	twitter: 'Twitter',
	whatsapp: 'WhatsApp',
	x: 'X',
	xmpp: 'XMPP',
	yahoo: 'Yahoo',
	yelp: 'Yelp',
};

/** El nombre de un servicio, o el que vino con la primera en mayúscula. */
export function serviceName(service: string): string {
	const normalized = service.trim().toLowerCase();
	if (!normalized) {
		return '';
	}
	return SERVICE_NAMES[normalized] ?? normalized.charAt(0).toUpperCase() + normalized.slice(1);
}
