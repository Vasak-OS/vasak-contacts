import type { Contact } from './address-book';
import { addressLines } from './contact-fields';

/**
 * Exportar la agenda a CSV, el formato que leen Google Contacts, Outlook y las
 * planillas. Es sólo del lado del cliente: el modelo `Contact` ya está
 * interpretado, no hace falta generar vCard (eso lo arma el almacén).
 *
 * El CSV no tiene un formato fijo —cada programa pone las columnas que quiere—,
 * así que se elige un juego de columnas claro y estable. Los campos de varios
 * valores (correos, teléfonos, webs) van en una celda, separados por ` ::: `,
 * como hace Google. Lo que importa al escribir: entrecomillar según RFC 4180 y
 * un BOM para que Excel abra el UTF-8 sin romper los acentos (el BOM lo agrega
 * quien escribe el archivo).
 */

/** El separador de varios valores dentro de una celda, como el de Google. */
const MULTI = ' ::: ';

/** Las columnas del CSV, en orden. El encabezado en inglés, como esperan los importadores. */
const COLUMNS: { header: string; value: (c: Contact) => string }[] = [
	{ header: 'Name', value: (c) => c.name },
	{ header: 'Nickname', value: (c) => c.nickname },
	{ header: 'Organization', value: (c) => c.organization },
	{ header: 'Title', value: (c) => c.title || c.role },
	{ header: 'Emails', value: (c) => c.emails.map((e) => e.value).join(MULTI) },
	{ header: 'Phones', value: (c) => c.phones.map((p) => p.value).join(MULTI) },
	{
		header: 'Addresses',
		value: (c) => c.addresses.map((a) => addressLines(a).join(', ')).join(MULTI),
	},
	{ header: 'Websites', value: (c) => c.websites.map((w) => w.value).join(MULTI) },
	{ header: 'Birthday', value: (c) => dateText(c.birthday) },
	{ header: 'Categories', value: (c) => c.categories.join(MULTI) },
	{ header: 'Notes', value: (c) => c.notes },
];

/** Una fecha parcial como texto: lo que se pueda, o lo crudo que trajo la tarjeta. */
function dateText(date: Contact['birthday']): string {
	if (!date) return '';
	if (date.year && date.month && date.day) {
		const mm = String(date.month).padStart(2, '0');
		const dd = String(date.day).padStart(2, '0');
		return `${date.year}-${mm}-${dd}`;
	}
	return date.text;
}

/**
 * Neutraliza la inyección de fórmulas: Excel y Google Sheets **ejecutan** una
 * celda que empieza con `=`, `+`, `-`, `@` o un tabulador/retorno —un contacto
 * llamado `=HYPERLINK(...)` correría al abrir el archivo—. Se antepone un
 * apóstrofo, que esos programas comen al mostrar. Sólo para los datos, no para
 * los encabezados, que son fijos y seguros.
 */
function neutralize(field: string): string {
	return /^[=+\-@\t\r]/.test(field) ? `'${field}` : field;
}

/**
 * Entrecomilla un campo si hace falta (RFC 4180): si lleva coma, comillas, o un
 * salto de línea, va entre comillas y las comillas de adentro se duplican. Las
 * celdas con saltos de línea —una nota de varios renglones— entran así en una
 * sola columna.
 */
function quote(field: string): string {
	if (/[",\r\n]/.test(field)) {
		return `"${field.replace(/"/g, '""')}"`;
	}
	return field;
}

/**
 * La agenda como CSV: una fila de encabezados y una por contacto. Las filas se
 * separan con CRLF, como pide RFC 4180 y esperan las planillas en Windows.
 *
 * No lleva BOM: lo agrega quien escribe el archivo, para no arrastrarlo si el
 * texto se usa para otra cosa.
 */
export function contactsToCsv(contacts: Contact[]): string {
	const header = COLUMNS.map((column) => quote(column.header)).join(',');
	const body = contacts.map((contact) =>
		COLUMNS.map((column) => quote(neutralize(column.value(contact)))).join(',')
	);
	return [header, ...body].join('\r\n');
}
