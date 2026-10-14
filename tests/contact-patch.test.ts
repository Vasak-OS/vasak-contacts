/**
 * El parche de crear y editar un contacto (vasak-contacts#2/#3).
 *
 * Es la regla que se equivoca **en silencio**: un campo que se manda cuando no
 * se tocó pisa lo que había, uno que se omite cuando se vació lo deja puesto.
 * Por eso se prueba campo por campo, sin la ventana.
 */

import { describe, expect, test } from 'bun:test';
import { emptyContact } from '@/tools/address-book';
import {
	buildContactPatch,
	type ContactFormState,
	emptyForm,
	formFromContact,
} from '@/tools/contact-patch';

/** Un formulario con lo que se le pase encima. */
function form(overrides: Partial<ContactFormState> = {}): ContactFormState {
	return emptyForm(overrides);
}

describe('el parche de un contacto', () => {
	test('un texto con valor se fija; vacío se saca con null', () => {
		const patch = buildContactPatch(
			form({ fullName: '  Ana Pérez  ', organization: '', note: '  ' }),
			{ mode: 'create' }
		);
		// El valor va recortado; lo vacío va `null`, que saca lo que hubiera.
		expect(patch.full_name).toBe('Ana Pérez');
		expect(patch.organization).toBeNull();
		expect(patch.note).toBeNull();
	});

	test('las listas fijan el conjunto entero; vacías van null al crear', () => {
		const patch = buildContactPatch(
			form({
				emails: [
					{ label: 'home', value: ' ana@casa.com ' },
					{ label: '', value: 'ana@work.com' },
					// La fila sin valor no entra.
					{ label: 'work', value: '  ' },
				],
			}),
			{ mode: 'create' }
		);
		expect(patch.emails).toEqual([
			// Valor recortado, etiqueta sólo si la hay.
			{ value: 'ana@casa.com', label: 'home' },
			{ value: 'ana@work.com' },
		]);
		// Sin teléfonos, al crear se mandan `null` (no hay nada que perder).
		expect(patch.phones).toBeNull();
	});

	test('los teléfonos llevan valor y etiqueta como los correos', () => {
		const patch = buildContactPatch(
			form({ phones: [{ label: 'cell', value: '+54 11 4444-4444' }] }),
			{ mode: 'create' }
		);
		expect(patch.phones).toEqual([{ value: '+54 11 4444-4444', label: 'cell' }]);
	});

	test('las relaciones llevan el vínculo en label y el nombre en value', () => {
		const patch = buildContactPatch(
			form({ related: [{ label: 'spouse', value: 'Juan' }] }),
			{ mode: 'create' }
		);
		expect(patch.related).toEqual([{ value: 'Juan', label: 'spouse' }]);
	});

	test('al crear lleva los seis campos y nunca un id', () => {
		const patch = buildContactPatch(form(), { mode: 'create' });
		expect(Object.keys(patch).sort()).toEqual([
			'emails',
			'full_name',
			'note',
			'organization',
			'phones',
			'related',
		]);
		expect(patch).not.toHaveProperty('id');
	});

	test('al editar lleva el id (número local)', () => {
		const patch = buildContactPatch(form({ fullName: 'Ana' }), { mode: 'edit', id: 42 });
		expect(patch.id).toBe(42);
		expect(patch.full_name).toBe('Ana');
	});

	test('al editar, relaciones vacías se OMITEN para no borrarlas a ciegas', () => {
		// El lado de lectura todavía no trae las relaciones; mandarlas `null`
		// borraría las que la persona tiene sin haberlas visto.
		const patch = buildContactPatch(form({ fullName: 'Ana' }), { mode: 'edit', id: 1 });
		expect(patch).not.toHaveProperty('related');
	});

	test('al editar, relaciones con valor sí se mandan', () => {
		const patch = buildContactPatch(
			form({ related: [{ label: 'child', value: 'Lola' }] }),
			{ mode: 'edit', id: 1 }
		);
		expect(patch.related).toEqual([{ value: 'Lola', label: 'child' }]);
	});

	test('al editar, un correo vaciado del todo se saca con null', () => {
		// La persona borró todos los correos: la lista vacía los saca.
		const patch = buildContactPatch(form({ fullName: 'Ana', emails: [] }), {
			mode: 'edit',
			id: 1,
		});
		expect(patch.emails).toBeNull();
	});
});

describe('el formulario desde un contacto', () => {
	test('precarga lo que el lado de lectura trae', () => {
		const contact = emptyContact({
			name: 'Ana Pérez',
			organization: 'Vasak',
			notes: 'una nota',
			emails: [{ label: 'work', value: 'ana@x.com' }],
			phones: [{ label: 'cell', value: '555' }],
		});
		const f = formFromContact(contact);
		expect(f.fullName).toBe('Ana Pérez');
		expect(f.organization).toBe('Vasak');
		expect(f.note).toBe('una nota');
		expect(f.emails).toEqual([{ label: 'work', value: 'ana@x.com' }]);
		expect(f.phones).toEqual([{ label: 'cell', value: '555' }]);
		// Las relaciones todavía no llegan: quedan vacías.
		expect(f.related).toEqual([]);
	});

	test('copia las listas, no las comparte con el contacto', () => {
		const contact = emptyContact({ emails: [{ label: 'home', value: 'a@b.com' }] });
		const f = formFromContact(contact);
		f.emails[0].value = 'otro@b.com';
		// Editar la fila del formulario no toca lo que vino del programa.
		expect(contact.emails[0].value).toBe('a@b.com');
	});
});
