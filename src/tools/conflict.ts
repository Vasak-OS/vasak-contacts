/**
 * La resolución de un conflicto de contacto, vista desde la ventana.
 *
 * Un conflicto es un cambio local que chocó con el servidor: el almacén lo
 * guardó aparte y espera a que la persona elija cuál queda. La ventana trae la
 * comparación con `get_contact_conflict` (el `GetConflict` del almacén) y
 * resuelve con `resolve_contact_conflict` (`ResolveConflict`).
 *
 * Esto es la parte que se equivoca **en silencio**: leer mal una vCard mostraría
 * la versión de otro, y la persona elegiría a ciegas. Por eso vive aparte de los
 * componentes y sin nada de Vue adentro, como `address-book.ts`, y se prueba
 * sola.
 *
 * ── v1: sin diff por campo ───────────────────────────────────────────────────
 *
 * No se marca campo por campo qué cambió: se muestran las **dos versiones
 * legibles** —nombre, organización, correos, teléfonos— y, si no se pudo leer
 * nada, el crudo formateado. Alcanza para decidir «la mía» o «la del servidor»
 * sin prometer un diff que todavía no está.
 */

/** Qué cambio local chocó con el servidor. */
export type ConflictOp = 'put' | 'delete';

/** Cómo resuelve la persona el conflicto. */
export type ConflictChoice = 'keep_local' | 'take_server';

/** Un contacto en conflicto, como lo lista `list_contact_conflicts`. */
export interface ConflictItem {
	/** El número local del contacto (su `store_id`). */
	id: string;
	op: ConflictOp;
	/** El nombre del contacto, para la fila. */
	summary: string;
	has_server_copy: boolean;
}

/** La comparación de un contacto en conflicto, de `get_contact_conflict`. */
export interface ConflictDetail {
	id: string;
	op: ConflictOp;
	/** La vCard local sin confirmar; `null` cuando el cambio es un borrado. */
	local: string | null;
	/** La vCard del servidor, o `null` si todavía no se trajo. */
	server: string | null;
	server_etag: string | null;
	/** Si no hay copia del servidor con qué comparar. */
	server_unavailable: boolean;
}

/** Un contacto leído de una vCard, lo justo para compararlo a ojo. */
export interface ReadableContact {
	name: string;
	organization: string;
	emails: string[];
	phones: string[];
}

/**
 * Junta las líneas plegadas de una vCard.
 *
 * El plegado normal de la 3.0/4.0: una continuación empieza con un espacio o una
 * tabulación, que se saca y se pega a la anterior. Sin esto un nombre largo
 * aparece cortado. No se maneja el `quoted-printable` de la 2.1 —eso lo hace el
 * intérprete de Rust que arma la ficha—: acá sólo hace falta algo legible para
 * comparar, y una tarjeta así cae al crudo formateado.
 */
function unfold(raw: string): string[] {
	const out: string[] = [];
	for (const line of raw.replace(/\r\n/g, '\n').split('\n')) {
		if ((line.startsWith(' ') || line.startsWith('\t')) && out.length > 0) {
			out[out.length - 1] += line.slice(1);
		} else {
			out.push(line);
		}
	}
	return out;
}

/**
 * Deshace las secuencias con barra de un valor de vCard: `\n` un salto, `\,`
 * una coma, `\;` un punto y coma, `\\` una barra. Lo que escribió quien hizo la
 * tarjeta se muestra, no se interpreta como nada más.
 */
function unescapeValue(value: string): string {
	return value.replace(/\\(.)/g, (_, char: string) => {
		switch (char) {
			case 'n':
			case 'N':
				return '\n';
			default:
				return char;
		}
	});
}

/** El nombre de la propiedad de una línea, en mayúscula, sin sus parámetros. */
function propertyName(line: string): string {
	const colon = line.indexOf(':');
	if (colon < 0) {
		return '';
	}
	const before = line.slice(0, colon);
	const semicolon = before.indexOf(';');
	return (semicolon < 0 ? before : before.slice(0, semicolon)).toUpperCase();
}

/** El valor de una línea: lo que va después del primer dos puntos. */
function propertyValue(line: string): string {
	const colon = line.indexOf(':');
	return colon < 0 ? '' : unescapeValue(line.slice(colon + 1)).trim();
}

/**
 * Lee una vCard a lo justo para compararla: nombre, organización, correos y
 * teléfonos. El nombre sale de `FN`; si no hay, se arma con `N` (apellido,
 * nombre). La organización es la primera parte de `ORG`.
 */
export function readableContact(raw: string): ReadableContact {
	let name = '';
	let structured = '';
	let organization = '';
	const emails: string[] = [];
	const phones: string[] = [];

	for (const line of unfold(raw)) {
		const name_ = propertyName(line);
		if (name_ === 'FN') {
			name = propertyValue(line) || name;
		} else if (name_ === 'N' && !structured) {
			// `N` es `apellido;nombre;…`: se arma «Nombre Apellido» para mostrar.
			const parts = line
				.slice(line.indexOf(':') + 1)
				.split(';')
				.map((p) => unescapeValue(p).trim());
			structured = [parts[1], parts[0]].filter((p) => p).join(' ');
		} else if (name_ === 'ORG') {
			organization =
				line
					.slice(line.indexOf(':') + 1)
					.split(';')
					.map((p) => unescapeValue(p).trim())
					.filter((p) => p)[0] ?? organization;
		} else if (name_ === 'EMAIL') {
			const value = propertyValue(line);
			if (value) {
				emails.push(value);
			}
		} else if (name_ === 'TEL') {
			const value = propertyValue(line);
			if (value) {
				phones.push(value);
			}
		}
	}

	return { name: name || structured, organization, emails, phones };
}

/**
 * El crudo formateado, para cuando no se pudo leer nada legible: las líneas
 * plegadas juntadas, sin las de armazón (`BEGIN`/`END`/`VERSION`) que no le
 * dicen nada a nadie.
 */
export function formattedRaw(raw: string): string {
	return unfold(raw)
		.map((line) => line.trimEnd())
		.filter((line) => {
			const name = propertyName(line);
			return line.trim() !== '' && name !== 'BEGIN' && name !== 'END' && name !== 'VERSION';
		})
		.join('\n');
}

/**
 * Si una lectura tiene algo que mostrar: un nombre, una organización, un correo
 * o un teléfono. Si no, la vista cae al crudo formateado.
 */
export function hasReadableFields(contact: ReadableContact): boolean {
	return (
		contact.name.length > 0 ||
		contact.organization.length > 0 ||
		contact.emails.length > 0 ||
		contact.phones.length > 0
	);
}
