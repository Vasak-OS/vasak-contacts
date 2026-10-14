/**
 * La insignia «sin sincronizar» de cada contacto, en la lista y en la ficha.
 *
 * Lo que se comprueba es que un contacto al día **no** lleve insignia, que uno
 * con un cambio sin subir y uno en conflicto lleven tonos distintos, y que la
 * lista y la ficha muestren lo mismo (las dos salen de `syncBadge`). Un tono
 * decidido en cada lado por separado es cómo terminan distintos.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { Badge } from '@vasakgroup/vue-libvasak';
import { mount, type VueWrapper } from '@vue/test-utils';
import ContactDetail from '@/components/contacts/ContactDetail.vue';
import ContactList from '@/components/contacts/ContactList.vue';
import { type Contact, emptyContact, type SyncState } from '@/tools/address-book';
import { syncBadge } from '@/tools/sync-badge';
import { olvidarTodo } from './dobles';

let wrapper: VueWrapper | null = null;

afterEach(() => {
	wrapper?.unmount();
	wrapper = null;
	olvidarTodo();
});

describe('syncBadge', () => {
	test('al día no lleva insignia', () => {
		expect(syncBadge('synced')).toBeNull();
	});

	test('un cambio sin subir va en «warning»; un conflicto en «error»', () => {
		expect(syncBadge('pending')?.tone).toBe('warning');
		expect(syncBadge('conflict')?.tone).toBe('error');
		// Tonos distintos: no se ven iguales.
		expect(syncBadge('pending')?.tone).not.toBe(syncBadge('conflict')?.tone);
	});
});

function contact(sync_state: SyncState, overrides: Partial<Contact> = {}): Contact {
	return emptyContact({ name: 'Ana', url: 'https://x/ana.vcf', sync_state, ...overrides });
}

describe('la insignia en la lista', () => {
	function list(contacts: Contact[]) {
		wrapper = mount(ContactList, {
			props: { contacts, selected: null, query: '', loading: false },
		});
		return wrapper;
	}

	test('un contacto al día no muestra insignia', () => {
		const view = list([contact('synced')]);
		expect(view.findComponent(Badge).exists()).toBe(false);
	});

	test('pending y conflict muestran insignia', () => {
		const pending = list([contact('pending')]);
		expect(pending.findComponent(Badge).exists()).toBe(true);
		pending.unmount();

		const conflict = list([contact('conflict', { url: 'https://x/b.vcf' })]);
		expect(conflict.findComponent(Badge).exists()).toBe(true);
	});
});

describe('la insignia en la ficha', () => {
	function detail(c: Contact | null) {
		wrapper = mount(ContactDetail, { props: { contact: c } });
		return wrapper;
	}

	test('al día no hay insignia', () => {
		const view = detail(contact('synced'));
		expect(view.find('[data-testid="sync-badge"]').exists()).toBe(false);
	});

	test('pending y conflict muestran la insignia en la ficha', () => {
		const pending = detail(contact('pending'));
		expect(pending.find('[data-testid="sync-badge"]').exists()).toBe(true);
		pending.unmount();

		const conflict = detail(contact('conflict'));
		expect(conflict.find('[data-testid="sync-badge"]').exists()).toBe(true);
	});
});
