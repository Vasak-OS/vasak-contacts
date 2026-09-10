/**
 * Cómo se arma el enlace que abre otra aplicación.
 *
 * Aparte y con tests porque son dos reglas cortas con una trampa cada una: una
 * de seguridad y otra de que el enlace simplemente no funcione.
 */

/**
 * El `mailto:` de una dirección.
 *
 * **La dirección va codificada.** Un `mailto:` acepta parámetros después de un
 * `?` —`?bcc=`, `?subject=`, `?body=`—, así que una dirección que traiga uno
 * abriría la ventana de redacción con un destinatario oculto puesto, o con un
 * cuerpo escrito. Y la tarjeta la escribió cualquiera: pudo llegar adjunta a un
 * correo o importada de un teléfono.
 *
 * Codificar convierte ese `?` en `%3F`, que es texto y no un separador. La
 * arroba también queda como `%40`, que es válido según el formato y los
 * clientes lo decodifican.
 */
export function enlaceDeCorreo(direccion: string): string {
	return `mailto:${encodeURIComponent(direccion.trim())}`;
}

/**
 * El `tel:` de un número.
 *
 * **Sin codificar y sin separadores**, que es lo contrario de lo anterior y por
 * un motivo distinto: el formato de un `tel:` sólo admite dígitos y un `+`
 * adelante. Codificar deja `tel:%2B54%2011%205555-1234`, donde el `+` ya no es
 * el prefijo internacional y el espacio no es nada — y el marcador o no abre o
 * llama a otro número.
 *
 * Así que se limpia en vez de escapar: se dejan los dígitos y el `+` inicial, y
 * se tira todo lo demás. Como lo que queda no puede contener un carácter con
 * significado en una URL, no hay nada que escapar.
 *
 * Devuelve vacío si no queda ningún dígito: un «número» que es sólo texto no se
 * puede marcar, y abrir `tel:` a secas le muestra a la persona un error del
 * sistema en vez de nada.
 */
export function enlaceDeTelefono(numero: string): string {
	const internacional = numero.trim().startsWith('+');
	const digitos = numero.replace(/\D/g, '');
	if (digitos.length === 0) {
		return '';
	}
	return `tel:${internacional ? '+' : ''}${digitos}`;
}
