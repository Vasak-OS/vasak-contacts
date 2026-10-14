/**
 * Resolver un conflicto de contacto (vasak-contacts#2, punto de conflicto).
 *
 * Tres cosas se prueban acá:
 *  - la ficha avisa el conflicto y pide abrir la comparación (`resolve`);
 *  - el diálogo trae la comparación, muestra las dos versiones, y cuando no hay
 *    copia del servidor ofrece reintentar en vez de fallar;
 *  - elegir «mantener la mía» o «tomar la del servidor» resuelve con la elección
 *    correcta y cierra, y un «no» del almacén se muestra sin cerrar.
 *
 * Los comandos (`get_contact_conflict` / `resolve_contact_conflict`) se prueban
 * por `useAgenda` con los dobles de `tests/dobles.ts`: que vayan con la cuenta y
 * el número local del contacto, y que resolver relea la agenda.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { ActionButton } from '@vasakgroup/vue-libvasak';
import { effectScope } from 'vue';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import ConflictDialog from '@/components/contacts/ConflictDialog.vue';
import ContactDetail from '@/components/contacts/ContactDetail.vue';
import { useAgenda } from '@/composables/use-agenda';
import { type Contact, emptyContact } from '@/tools/address-book';
import type { ConflictChoice, ConflictDetail } from '@/tools/conflict';
import { STORE_PERMISSION_DENIED } from '@/tools/store-permission';
import { answerCommand, invokedCommands, olvidarTodo } from './dobles';

let wrapper: VueWrapper | null = null;

afterEach(() => {
	wrapper?.unmount();
	wrapper = null;
	olvidarTodo();
});

/** El botón de un diálogo por su etiqueta (en las pruebas, la clave de i18n). */
function button(w: VueWrapper, label: string) {
	return w.findAllComponents(ActionButton).find((b) => b.props('label') === label);
}

const CONFLICTED = emptyContact({
	name: 'Ana Pérez',
	url: 'https://x/ana.vcf',
	account_id: 'a1',
	store_id: '7',
	sync_state: 'conflict',
});

function detailOf(op: 'put' | 'delete', over: Partial<ConflictDetail> = {}): ConflictDetail {
	return {
		id: '7',
		op,
		local: op === 'delete' ? null : 'BEGIN:VCARD\r\nFN:Ana local\r\nEND:VCARD\r\n',
		server: 'BEGIN:VCARD\r\nFN:Ana servidor\r\nEND:VCARD\r\n',
		server_etag: 'etag-srv',
		server_unavailable: false,
		...over,
	};
}

// ── La ficha avisa el conflicto ──────────────────────────────────────────────

describe('el banner de conflicto en la ficha', () => {
	test('un contacto en conflicto muestra el banner y pide resolver', async () => {
		wrapper = mount(ContactDetail, { props: { contact: CONFLICTED } });
		expect(wrapper.find('[data-testid="conflict-banner"]').exists()).toBe(true);

		await button(wrapper, 'conflict.resolve')?.trigger('click');
		const resolved = wrapper.emitted('resolve');
		expect(resolved).toBeTruthy();
		expect((resolved![0] as [Contact])[0].url).toBe(CONFLICTED.url);
	});

	test('un contacto al día no muestra el banner', () => {
		wrapper = mount(ContactDetail, {
			props: { contact: emptyContact({ name: 'Ana', sync_state: 'synced' }) },
		});
		expect(wrapper.find('[data-testid="conflict-banner"]').exists()).toBe(false);
	});
});

// ── El diálogo de la comparación ─────────────────────────────────────────────

describe('el diálogo de conflicto', () => {
	function dialog(opts: {
		load: (c: Contact) => Promise<ConflictDetail>;
		resolve?: (c: Contact, choice: ConflictChoice) => Promise<string | null>;
	}) {
		wrapper = mount(ConflictDialog, {
			attachTo: document.body.appendChild(document.createElement('div')),
			props: {
				open: true,
				contact: CONFLICTED,
				load: opts.load,
				resolve: opts.resolve ?? (async () => null),
			},
		});
		return wrapper;
	}

	test('trae la comparación y muestra las dos versiones', async () => {
		dialog({ load: async () => detailOf('put') });
		await flushPromises();
		expect(document.body.querySelector('[data-testid="mine"]')).not.toBeNull();
		expect(document.body.querySelector('[data-testid="theirs"]')).not.toBeNull();
		const text = document.body.textContent ?? '';
		expect(text).toContain('Ana local');
		expect(text).toContain('Ana servidor');
	});

	test('un borrado local dice que «la mía» es borrarlo', async () => {
		dialog({ load: async () => detailOf('delete') });
		await flushPromises();
		expect(document.body.querySelector('[data-testid="mine-delete"]')).not.toBeNull();
	});

	test('sin copia del servidor, ofrece reintentar y no deja tomarla', async () => {
		let calls = 0;
		const w = dialog({
			load: async () => {
				calls += 1;
				return detailOf('put', { server: null, server_unavailable: true });
			},
		});
		await flushPromises();
		expect(document.body.querySelector('[data-testid="server-unavailable"]')).not.toBeNull();
		// «Tomar la del servidor» está deshabilitado: no hay qué tomar.
		expect(button(w, 'conflict.takeTheirs')?.props('disabled')).toBe(true);
		// «Mantener la mía» sí se puede.
		expect(button(w, 'conflict.keepMine')?.props('disabled')).toBe(false);

		await button(w, 'conflict.retry')?.trigger('click');
		await flushPromises();
		expect(calls).toBe(2);
	});

	test('«mantener la mía» resuelve con keep_local y cierra', async () => {
		const choices: ConflictChoice[] = [];
		const w = dialog({
			load: async () => detailOf('put'),
			resolve: async (_c, choice) => {
				choices.push(choice);
				return null;
			},
		});
		await flushPromises();
		await button(w, 'conflict.keepMine')?.trigger('click');
		await flushPromises();
		expect(choices).toEqual(['keep_local']);
		// Al resolver bien, pide cerrarse.
		expect(w.emitted('update:open')?.at(-1)).toEqual([false]);
	});

	test('«tomar la del servidor» resuelve con take_server', async () => {
		const choices: ConflictChoice[] = [];
		const w = dialog({
			load: async () => detailOf('put'),
			resolve: async (_c, choice) => {
				choices.push(choice);
				return null;
			},
		});
		await flushPromises();
		await button(w, 'conflict.takeTheirs')?.trigger('click');
		await flushPromises();
		expect(choices).toEqual(['take_server']);
	});

	test('un «no» del almacén al resolver se muestra y no cierra', async () => {
		const w = dialog({
			load: async () => detailOf('put'),
			resolve: async () => STORE_PERMISSION_DENIED,
		});
		await flushPromises();
		await button(w, 'conflict.keepMine')?.trigger('click');
		await flushPromises();
		// No pidió cerrarse.
		expect(w.emitted('update:open')).toBeUndefined();
		// Y muestra el aviso traducido (en las pruebas, la clave).
		expect(document.body.textContent).toContain('conflict.denied');
	});

	test('si no se pudo traer la comparación, lo dice y no deja elegir', async () => {
		const w = dialog({
			load: async () => {
				throw new Error('se cortó');
			},
		});
		await flushPromises();
		expect(button(w, 'conflict.keepMine')?.props('disabled')).toBe(true);
		expect(button(w, 'conflict.takeTheirs')?.props('disabled')).toBe(true);
	});
});

// ── Los comandos, por useAgenda ──────────────────────────────────────────────

describe('getConflict y resolveConflict', () => {
	function withAgenda<T>(fn: (agenda: ReturnType<typeof useAgenda>) => Promise<T>): Promise<T> {
		const scope = effectScope();
		return scope.run(async () => {
			const agenda = useAgenda(() => 'es');
			try {
				return await fn(agenda);
			} finally {
				scope.stop();
			}
		}) as Promise<T>;
	}

	test('getConflict va con la cuenta y el número local del contacto', async () => {
		answerCommand('get_contact_conflict', () => detailOf('put'));
		await withAgenda(async (agenda) => {
			const detail = await agenda.getConflict(CONFLICTED);
			expect(detail.id).toBe('7');
		});
		const call = invokedCommands.find((c) => c.command === 'get_contact_conflict');
		expect(call?.args).toMatchObject({ accountId: 'a1', id: '7' });
	});

	test('resolveConflict manda la elección y relee la agenda', async () => {
		answerCommand('resolve_contact_conflict', () => undefined);
		answerCommand('listar_cuentas', () => []);
		await withAgenda(async (agenda) => {
			const error = await agenda.resolveConflict(CONFLICTED, 'take_server');
			expect(error).toBeNull();
		});
		const call = invokedCommands.find((c) => c.command === 'resolve_contact_conflict');
		expect(call?.args).toMatchObject({ accountId: 'a1', id: '7', choice: 'take_server' });
		// Releyó: pidió las cuentas de nuevo.
		expect(invokedCommands.some((c) => c.command === 'listar_cuentas')).toBe(true);
	});

	test('un error al resolver vuelve como texto y no relee', async () => {
		answerCommand('resolve_contact_conflict', () => {
			throw new Error(STORE_PERMISSION_DENIED);
		});
		await withAgenda(async (agenda) => {
			const error = await agenda.resolveConflict(CONFLICTED, 'keep_local');
			expect(error).toContain(STORE_PERMISSION_DENIED);
		});
		expect(invokedCommands.some((c) => c.command === 'listar_cuentas')).toBe(false);
	});
});
