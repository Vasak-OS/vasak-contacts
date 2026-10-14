/**
 * El formulario de crear y editar un contacto (vasak-contacts#2/#3).
 *
 * El parche en sí se prueba en `contact-patch.test.ts`; acá se prueba el
 * cableado del formulario: que al crear resuelva la libreta elegida y emita el
 * parche armado, y que no deje guardar sin nombre.
 */

import { describe, expect, test } from 'bun:test';
import { ActionButton, TextInput } from '@vasakgroup/vue-libvasak';
import { mount } from '@vue/test-utils';
import ContactForm from '@/components/contacts/ContactForm.vue';
import { emptyContact } from '@/tools/address-book';
import type { AddressBook } from '@/tools/library-filter';

const BOOK: AddressBook = {
	accountId: 'a1',
	accountName: 'ana@nube',
	id: '1',
	name: 'Personal',
	key: 'a1\u00001',
};

function form(props: Partial<InstanceType<typeof ContactForm>['$props']> = {}) {
	return mount(ContactForm, {
		attachTo: document.body.appendChild(document.createElement('div')),
		props: { open: true, contact: null, addressBooks: [BOOK], ...props },
	});
}

/** El botón de un diálogo por su etiqueta. */
function button(wrapper: ReturnType<typeof form>, label: string) {
	return wrapper.findAllComponents(ActionButton).find((b) => b.props('label') === label);
}

describe('el formulario de contacto', () => {
	test('al crear, emite el parche con la cuenta y la libreta elegidas', async () => {
		const wrapper = form();
		// El primer campo de texto es el nombre.
		await wrapper.findAllComponents(TextInput)[0].setValue('Ana Pérez');

		await button(wrapper, 'form.create')?.trigger('click');

		const submitted = wrapper.emitted('submit');
		expect(submitted).toBeTruthy();
		const [payload] = submitted![0] as [Record<string, unknown>];
		expect(payload).toMatchObject({
			accountId: 'a1',
			addressBookId: '1',
			ifMatch: '',
		});
		expect((payload.payload as Record<string, unknown>).full_name).toBe('Ana Pérez');
		wrapper.unmount();
	});

	test('sin nombre no deja crear', async () => {
		const wrapper = form();

		await button(wrapper, 'form.create')?.trigger('click');

		expect(wrapper.emitted('submit')).toBeUndefined();
		wrapper.unmount();
	});

	test('al editar, emite el ETag y el id del contacto', async () => {
		const contact = emptyContact({
			name: 'Ana Pérez',
			account_id: 'a1',
			address_book_id: '1',
			store_id: '7',
			etag: 'e-1',
		});
		const wrapper = form({ contact });

		await button(wrapper, 'form.save')?.trigger('click');

		const submitted = wrapper.emitted('submit');
		expect(submitted).toBeTruthy();
		const [payload] = submitted![0] as [Record<string, unknown>];
		expect(payload).toMatchObject({ accountId: 'a1', addressBookId: '1', ifMatch: 'e-1' });
		expect((payload.payload as Record<string, unknown>).id).toBe(7);
		wrapper.unmount();
	});
});
