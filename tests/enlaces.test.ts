import { describe, expect, test } from 'bun:test';
import { enlaceDeCorreo, enlaceDeTelefono } from '../src/tools/enlaces';

describe('enlaceDeCorreo', () => {
	test('una dirección normal se abre', () => {
		expect(enlaceDeCorreo('ana@ejemplo.com')).toBe('mailto:ana%40ejemplo.com');
	});

	test('una dirección con parámetros no puede agregar destinatarios ocultos', () => {
		// Un `mailto:` acepta `?bcc=` después de la dirección. Una tarjeta la
		// escribió cualquiera —pudo llegar adjunta a un correo—, así que sin
		// codificar, abrir el contacto prepararía un mensaje con un
		// destinatario que no se ve.
		const enlace = enlaceDeCorreo('ana@ejemplo.com?bcc=espia@ajeno.com');

		expect(enlace).not.toContain('?');
		expect(enlace).toContain('%3F');
	});

	test('tampoco puede escribir el cuerpo ni el asunto', () => {
		for (const veneno of [
			'ana@x.com?subject=Urgente',
			'ana@x.com?body=Mandame%20la%20clave',
			'ana@x.com&cc=otro@y.com',
		]) {
			const enlace = enlaceDeCorreo(veneno);
			expect(enlace).not.toContain('?');
			expect(enlace).not.toContain('&');
		}
	});
});

describe('enlaceDeTelefono', () => {
	test('se sacan los separadores y se conserva el prefijo', () => {
		// Codificar dejaría `tel:%2B54%2011%205555-1234`, donde el `+` ya no es
		// el prefijo internacional: el marcador o no abre o llama a otro número.
		expect(enlaceDeTelefono('+54 11 5555-1234')).toBe('tel:+541155551234');
		expect(enlaceDeTelefono('(011) 5555-1234')).toBe('tel:01155551234');
		expect(enlaceDeTelefono('11.5555.1234')).toBe('tel:1155551234');
	});

	test('el enlace no lleva nada que haya que escapar', () => {
		const enlace = enlaceDeTelefono('+54 11 5555-1234');
		expect(enlace).toBe(encodeURI(enlace));
		expect(enlace).not.toContain('%');
	});

	test('un número que no tiene dígitos no se marca', () => {
		// Abrir `tel:` a secas le muestra a la persona un error del sistema en
		// vez de nada.
		expect(enlaceDeTelefono('')).toBe('');
		expect(enlaceDeTelefono('llamar al fijo')).toBe('');
		expect(enlaceDeTelefono('---')).toBe('');
	});

	test('un número sin prefijo no se lo inventa', () => {
		// Poner un `+` donde no estaba convierte un número local en uno
		// internacional, y llama a otro lado.
		expect(enlaceDeTelefono('5555-1234')).toBe('tel:55551234');
	});
});
