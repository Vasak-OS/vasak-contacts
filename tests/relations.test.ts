import { describe, expect, test } from 'bun:test';
import { type Contact, emptyContact, type Relationship } from '../src/tools/address-book';
import { normalizeUid, resolveRelated } from '../src/tools/relations';

function rel(parts: Partial<Relationship>): Relationship {
	return { kind: 'friend', name: '', target_uid: '', ...parts };
}

function person(parts: Partial<Contact>): Contact {
	return emptyContact({ uid: 'yo', name: 'Yo', ...parts });
}

describe('normalizeUid', () => {
	test('quita el esquema urn:uuid:, en minúsculas o mayúsculas', () => {
		expect(normalizeUid('urn:uuid:abc-123')).toBe('abc-123');
		expect(normalizeUid('URN:UUID:abc-123')).toBe('abc-123');
	});

	test('un uid pelado queda como está', () => {
		expect(normalizeUid('abc-123')).toBe('abc-123');
	});
});

describe('resolveRelated', () => {
	test('una relación por uid se resuelve a la ficha cargada, con su nombre', () => {
		const ana = person({ uid: 'ana', name: 'Ana Pérez' });
		const me = person({ related: [rel({ target_uid: 'ana', name: 'otro nombre' })] });
		const [r] = resolveRelated(me, [me, ana]);
		expect(r.contact).toBe(ana);
		expect(r.name).toBe('Ana Pérez');
		expect(r.kind).toBe('friend');
	});

	test('el uid se compara normalizado: urn en la ficha y pelado en la relación', () => {
		const ana = person({ uid: 'urn:uuid:X', name: 'Ana' });
		const me = person({ related: [rel({ target_uid: 'X' })] });
		expect(resolveRelated(me, [ana])[0].contact).toBe(ana);
	});

	test('el uid se compara normalizado: pelado en la ficha y urn en la relación', () => {
		const ana = person({ uid: 'X', name: 'Ana' });
		const me = person({ related: [rel({ target_uid: 'URN:UUID:X' })] });
		expect(resolveRelated(me, [ana])[0].contact).toBe(ana);
	});

	test('una relación de texto no enlaza a nadie y conserva su nombre', () => {
		const me = person({ related: [rel({ name: 'Mi vecina' })] });
		const [r] = resolveRelated(me, [me, person({ uid: 'ana' })]);
		expect(r.contact).toBeNull();
		expect(r.name).toBe('Mi vecina');
	});

	test('un target_uid que no está entre los cargados no enlaza', () => {
		const me = person({ related: [rel({ target_uid: 'fantasma', name: 'Fulano' })] });
		const [r] = resolveRelated(me, [me, person({ uid: 'ana' })]);
		expect(r.contact).toBeNull();
		expect(r.name).toBe('Fulano');
	});

	test('con el uid repetido gana siempre la primera ficha', () => {
		const primera = person({ uid: 'dup', name: 'Primera' });
		const segunda = person({ uid: 'urn:uuid:dup', name: 'Segunda' });
		const me = person({ related: [rel({ target_uid: 'dup' })] });
		expect(resolveRelated(me, [primera, segunda])[0].contact).toBe(primera);
		expect(resolveRelated(me, [segunda, primera])[0].contact).toBe(segunda);
	});

	test('no se enlaza a sí mismo', () => {
		const me = person({ uid: 'urn:uuid:yo', related: [rel({ target_uid: 'yo', name: 'Yo mismo' })] });
		const [r] = resolveRelated(me, [me]);
		expect(r.contact).toBeNull();
		expect(r.name).toBe('Yo mismo');
	});
});
