/** La fusión local en la agenda: unir, no volver a proponer, separar y recordar. */

import { afterEach, describe, expect, test } from 'bun:test';
import { type EffectScope, effectScope } from 'vue';
import { useAgenda } from '@/composables/use-agenda';
import { emptyContact } from '@/tools/address-book';
import { loadMerges } from '@/tools/merge';
import { answerCommand, olvidarTodo } from './dobles';

const mk = (url: string, name: string) =>
	emptyContact({
		name,
		sort_key: name,
		url,
		uid: url,
		emails: [{ label: '', value: 'ana@x.com' }],
	});
const A1 = mk('store:a1:https://x/1.vcf', 'Ana Pérez');
const A2 = mk('store:a1:https://x/2.vcf', 'Ana Pérez');
const OTRA = emptyContact({ name: 'Zeta', sort_key: 'Zeta', url: 'store:a1:https://x/3.vcf' });

let scope: EffectScope | null = null;
afterEach(() => {
	scope?.stop();
	scope = null;
	olvidarTodo();
	globalThis.localStorage?.clear();
});

async function agenda() {
	answerCommand('listar_cuentas', () => [
		{ id: 'a1', nombre: 'ana@nube.ejemplo.com', necesita_reconectarse: false },
	]);
	answerCommand('contactos_de_la_cuenta', () => ({
		libretas: [],
		contactos: [A1, A2, OTRA],
		fallos: [],
	}));
	scope = effectScope();
	const ag = scope.run(() => useAgenda(() => 'es'));
	if (!ag) throw new Error('sin agenda');
	await ag.cargar();
	return ag;
}

describe('fusión local en la agenda', () => {
	test('mergeGroup deja una sola tarjeta unida, seleccionada, y ya no se propone', async () => {
		const ag = await agenda();
		const group = ag.duplicateGroupOf(ag.visibles.value.find((c) => c.url === A1.url) ?? A1);
		expect(group.length).toBe(2);
		expect(ag.visibles.value.length).toBe(3);

		ag.mergeGroup(group);

		expect(ag.visibles.value.length).toBe(2);
		expect(ag.elegido.value?.url).toBe(A1.url);
		expect(ag.isMerged(ag.elegido.value ?? A1)).toBe(true);
		expect(ag.mergeMembersOf(ag.elegido.value ?? A1).length).toBe(2);
		expect(ag.duplicateCount.value).toBe(0);
		expect(ag.duplicateGroupOf(ag.elegido.value ?? A1)).toEqual([]);
	});

	test('unmerge las vuelve a separar', async () => {
		const ag = await agenda();
		const first = ag.visibles.value.find((c) => c.url === A1.url) ?? A1;
		ag.mergeGroup(ag.duplicateGroupOf(first));
		ag.unmerge(ag.elegido.value ?? first);

		expect(ag.visibles.value.length).toBe(3);
		expect(ag.isMerged(first)).toBe(false);
		expect(loadMerges()).toEqual([]);
	});

	test('la fusión se recuerda en el almacenamiento y una agenda nueva la aplica', async () => {
		const ag = await agenda();
		ag.mergeGroup(ag.duplicateGroupOf(ag.visibles.value.find((c) => c.url === A1.url) ?? A1));
		expect(loadMerges()).toEqual([[A1.url, A2.url]]);

		scope?.stop();
		const otra = await agenda();
		expect(otra.visibles.value.length).toBe(2);
	});
});
