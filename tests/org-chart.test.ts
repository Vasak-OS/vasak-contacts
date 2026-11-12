import { describe, expect, test } from 'bun:test';
import { type Contact, emptyContact, type Relationship } from '../src/tools/address-book';
import { orgChartFor } from '../src/tools/org-chart';

const ORG = 'Vasak Group';

function rel(kind: string, target_uid: string): Relationship {
	return { kind, name: '', target_uid };
}

function p(uid: string, related: Relationship[] = [], organization = ORG): Contact {
	return emptyContact({ uid, name: uid, organization, related });
}

const shape = (nodes: ReturnType<typeof orgChartFor>) =>
	nodes.map((n) => [n.contact.uid, n.relation, n.depth]);

describe('orgChartFor', () => {
	// jefa <- ana (manager: jefa) ; ana tiene asistente luis ; beto es otro reporte de jefa
	const jefa = p('jefa');
	const ana = p('ana', [rel('manager', 'jefa'), rel('assistant', 'luis')]);
	const beto = p('beto', [rel('manager', 'jefa')]);
	const luis = p('luis');
	const all = [jefa, ana, luis, beto];

	test('el árbol sale en preorden con la relación y la profundidad de cada uno', () => {
		expect(shape(orgChartFor(jefa, all))).toEqual([
			['jefa', 'root', 0],
			['ana', 'report', 1],
			['luis', 'assistant', 2],
			['beto', 'report', 1],
		]);
	});

	test('sólo el contacto pedido va marcado como actual', () => {
		const nodes = orgChartFor(ana, all);
		expect(nodes.filter((n) => n.current).map((n) => n.contact.uid)).toEqual(['ana']);
	});

	test('desde una hoja se sube hasta la raíz y sale el árbol completo', () => {
		expect(orgChartFor(luis, all).map((n) => n.contact.uid)).toEqual(['jefa', 'ana', 'luis', 'beto']);
	});

	test('una relación hacia otra organización no entra', () => {
		const ajeno = p('ajeno', [], 'Otra SA');
		const yo = p('yo', [rel('manager', 'ajeno')]);
		expect(orgChartFor(yo, [yo, ajeno])).toEqual([]);
		const jefe = p('jefe', [], ORG);
		const yo2 = p('yo2', [rel('manager', 'jefe'), rel('manager', 'ajeno')]);
		expect(orgChartFor(yo2, [yo2, jefe, ajeno]).map((n) => n.contact.uid)).toEqual(['jefe', 'yo2']);
	});

	test('la organización se compara sin mayúsculas ni espacios de más', () => {
		const jefe = p('jefe', [], '  vasak GROUP ');
		const yo = p('yo', [rel('manager', 'jefe')]);
		expect(orgChartFor(yo, [yo, jefe]).map((n) => n.contact.uid)).toEqual(['jefe', 'yo']);
	});

	test('un contacto sin organización no tiene organigrama', () => {
		const jefe = p('jefe', [], '');
		const yo = p('yo', [rel('manager', 'jefe')], '');
		expect(orgChartFor(yo, [yo, jefe])).toEqual([]);
	});

	test('un target_uid que no resuelve y una relación a sí mismo no arman árbol', () => {
		const yo = p('yo', [rel('manager', 'fantasma'), rel('manager', 'yo'), rel('assistant', 'yo')]);
		expect(orgChartFor(yo, [yo])).toEqual([]);
	});

	test('sin vínculos de jefe ni asistente no hay organigrama', () => {
		const yo = p('yo', [rel('friend', 'otro')]);
		expect(orgChartFor(yo, [yo, p('otro')])).toEqual([]);
	});

	test('urn:uuid: engancha con el uid pelado, de un lado y del otro', () => {
		const jefeUrn = p('urn:uuid:j1');
		const yo = p('yo', [rel('manager', 'j1')]);
		expect(orgChartFor(yo, [yo, jefeUrn]).map((n) => n.contact.uid)).toEqual(['urn:uuid:j1', 'yo']);

		const jefe = p('j2');
		const yoUrn = p('urn:uuid:y2', [rel('manager', 'j2')]);
		expect(orgChartFor(yoUrn, [yoUrn, jefe]).map((n) => n.contact.uid)).toEqual(['j2', 'urn:uuid:y2']);
	});

	test('un hijo con dos jefes cuelga del primero que se declara', () => {
		const a = p('a');
		const b = p('b');
		const hijo = p('hijo', [rel('manager', 'a'), rel('manager', 'b')]);
		expect(shape(orgChartFor(hijo, [a, b, hijo]))).toEqual([
			['a', 'root', 0],
			['hijo', 'report', 1],
		]);
	});

	test('un ciclo de jefes termina y muestra a los dos', () => {
		const a = p('a', [rel('manager', 'b')]);
		const b = p('b', [rel('manager', 'a')]);
		const nodes = orgChartFor(a, [a, b]);
		expect(nodes.map((n) => n.contact.uid).sort()).toEqual(['a', 'b']);
		expect(nodes).toHaveLength(2);
	});
});
