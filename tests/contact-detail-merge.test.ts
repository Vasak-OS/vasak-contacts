/** Los avisos de unir y separar tarjetas en la ficha (vasak-contacts#5). */

import { afterEach, describe, expect, test } from 'bun:test';
import I18n from '@vasakgroup/tauri-plugin-i18n';
import { mount, type VueWrapper } from '@vue/test-utils';
import ContactDetail from '@/components/contacts/ContactDetail.vue';
import { type Contact, emptyContact } from '@/tools/address-book';
import { olvidarTodo } from './dobles';

let wrapper: VueWrapper | null = null;
afterEach(() => {
	wrapper?.unmount();
	wrapper = null;
	olvidarTodo();
});

const ANA = emptyContact({ name: 'Ana Pérez', url: 'https://x/ana.vcf' });
const ANITA = emptyContact({ name: 'Anita Pérez', url: 'https://x/anita.vcf' });

function show(extra: { duplicateGroup?: Contact[]; mergeMembers?: Contact[] } = {}) {
	wrapper = mount(ContactDetail, { props: { contact: ANA, ...extra } });
	return wrapper;
}

describe('avisos de fusión en la ficha', () => {
	test('sin grupo ni miembros no hay ningún aviso', () => {
		const view = show();
		expect(view.find('[data-testid="merge-banner"]').exists()).toBe(false);
		expect(view.find('[data-testid="merged-banner"]').exists()).toBe(false);
	});

	test('con repetidos propone unir y emite merge con el grupo', async () => {
		const view = show({ duplicateGroup: [ANA, ANITA] });
		const banner = view.find('[data-testid="merge-banner"]');
		expect(banner.exists()).toBe(true);
		expect(view.find('[data-testid="merged-banner"]').exists()).toBe(false);
		await banner.find('button').trigger('click');
		expect(view.emitted('merge')?.[0]).toEqual([[ANA, ANITA]]);
	});

	test('un grupo de uno solo no propone unir', () => {
		expect(show({ duplicateGroup: [ANA] }).find('[data-testid="merge-banner"]').exists()).toBe(false);
	});

	test('una ficha unida lista los nombres y emite unmerge con el contacto', async () => {
		await I18n.getInstance().load();
		const view = show({ mergeMembers: [ANA, ANITA] });
		const banner = view.find('[data-testid="merged-banner"]');
		expect(banner.exists()).toBe(true);
		expect(banner.text()).toContain('Ana Pérez');
		expect(banner.text()).toContain('Anita Pérez');
		await banner.find('button').trigger('click');
		expect(view.emitted('unmerge')?.[0]).toEqual([ANA]);
	});
});
