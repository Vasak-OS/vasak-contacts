/**
 * Ordenar y buscar en la agenda.
 *
 * Aparte de los componentes y sin nada de Vue adentro. Es lo único de una
 * libreta que se equivoca **en silencio**: un orden que manda a los Álvarez al
 * final se ve prolijo, y una búsqueda que no encuentra a «Perez» cuando el
 * contacto es «Pérez» simplemente dice que no hay nadie.
 */

/** Un dato con etiqueta: un correo, un teléfono. */
export interface Dato {
	/** «casa», «trabajo», «celular»… o vacío si la tarjeta no dijo. */
	tipo: string;
	valor: string;
}

/** Un contacto tal como llega del programa. */
export interface Contacto {
	uid: string;
	nombre: string;
	/** «Pérez, Ana»: con qué ordenarlo. Ver `ordenados`. */
	orden: string;
	correos: Dato[];
	telefonos: Dato[];
	organizacion: string;
	notas: string;
	url: string;
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
export function ordenados(contactos: Contacto[], locale: string): Contacto[] {
	const comparador = new Intl.Collator(locale, { sensitivity: 'base', numeric: true });
	// Sobre una copia: ordenar el arreglo original mutaría lo que vino del
	// programa, y en Vue eso dispara cada dependencia que lo esté mirando.
	return [...contactos].sort((a, b) => comparador.compare(a.orden, b.orden));
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
export function sinAcentos(texto: string): string {
	return texto.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
}

/**
 * Los contactos que coinciden con lo que se escribió.
 *
 * Se busca en el nombre, en la organización, en los correos y en los teléfonos:
 * la gente busca por lo que recuerda, y lo que recuerda a veces es «el que
 * trabaja en Vasak» o los últimos cuatro dígitos de un número.
 *
 * Cada palabra tiene que aparecer en **alguna** parte, no todas en la misma:
 * escribir «ana vasak» encuentra a Ana que trabaja en Vasak, que es lo que
 * quiere decir quien lo escribe.
 */
export function buscados(contactos: Contacto[], consulta: string): Contacto[] {
	const palabras = sinAcentos(consulta)
		.split(/\s+/)
		.filter((p) => p.length > 0);
	if (palabras.length === 0) {
		return contactos;
	}

	return contactos.filter((contacto) => {
		const dondeBuscar = sinAcentos(
			[
				contacto.nombre,
				contacto.organizacion,
				...contacto.correos.map((c) => c.valor),
				// Los teléfonos, **sólo sus dígitos**: nadie busca «11-5555»
				// escribiendo el guión en el mismo lugar, y los separadores que
				// usa la gente no son una lista corta — enumerar el guión, el
				// punto y el paréntesis dejaba afuera «11/5555-1234».
				...contacto.telefonos.map((t) => t.valor.replace(/\D/g, '')),
			].join(' ')
		);

		return palabras.every((palabra) => {
			if (dondeBuscar.includes(palabra)) {
				return true;
			}
			// Y si la palabra tiene dígitos, también por sus dígitos solos: así
			// «(11) 5555-1234» encuentra un número guardado como «11/5555 1234».
			//
			// **Sólo si quedan dígitos.** Una palabra sin ninguno queda en la
			// cadena vacía, y todo texto la contiene: sin este control, buscar
			// «nadie» devolvía la agenda entera.
			const soloDigitos = palabra.replace(/\D/g, '');
			return soloDigitos.length > 0 && dondeBuscar.includes(soloDigitos);
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
export function inicialDe(contacto: Contacto): string {
	const primera = sinAcentos(contacto.orden.trim()).charAt(0).toUpperCase();
	return /[A-Z]/.test(primera) ? primera : '#';
}
