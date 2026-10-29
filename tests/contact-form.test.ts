/**
 * El formulario de crear y editar un contacto (vasak-contacts#2/#3).
 *
 * El parche en sí se prueba en `contact-patch.test.ts`; acá se prueba el
 * cableado del formulario: que al crear resuelva la libreta elegida y emita el
 * parche armado, y que no deje guardar sin nombre.
 */

import { describe, expect, test } from 'bun:test';
import { ActionButton, SelectField, TextInput } from '@vasakgroup/vue-libvasak';
import { mount } from '@vue/test-utils';
import ContactForm from '@/components/contacts/ContactForm.vue';
import { type Address, emptyContact } from '@/tools/address-book';
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

/** El placeholder de un campo de texto (la clave i18n que identifica la parte). */
function placeholderOf(input: { props: () => unknown }): string {
	return (input.props() as { placeholder: string }).placeholder;
}

/** Los componentes (de un tipo) que viven dentro del bloque de direcciones. */
function inAddresses(
	wrapper: ReturnType<typeof form>,
	component: typeof TextInput | typeof SelectField
) {
	// El diálogo se teletransporta al body: se busca ahí, no dentro del wrapper.
	const fieldset = document.querySelector('[data-testid="address-fields"]');
	return wrapper.findAllComponents(component).filter((c) => fieldset?.contains(c.element));
}

const ADDRESS: Address = {
	label: 'work',
	po_box: '',
	extended: '',
	street: 'Calle 1',
	locality: 'Rosario',
	region: '',
	postal_code: '',
	country: '',
};

/** El parche que emite el formulario al guardar. */
async function savedPatch(wrapper: ReturnType<typeof form>, label: string) {
	await button(wrapper, label)?.trigger('click');
	const [payload] = wrapper.emitted('submit')![0] as [{ payload: Record<string, unknown> }];
	return payload.payload;
}

describe('las direcciones del formulario', () => {
	test('«Agregar dirección» suma un bloque y quitar lo saca', async () => {
		const wrapper = form();
		expect(inAddresses(wrapper, SelectField)).toHaveLength(0);

		await button(wrapper, 'form.addAddress')?.trigger('click');
		await button(wrapper, 'form.addAddress')?.trigger('click');
		// Cada bloque trae su selector de etiqueta y las siete partes.
		expect(inAddresses(wrapper, SelectField)).toHaveLength(2);
		expect(inAddresses(wrapper, TextInput)).toHaveLength(14);

		const remove = wrapper
			.findAllComponents(ActionButton)
			.filter((b) => b.props('label') === 'form.remove')
			.find((b) =>
				document.querySelector('[data-testid="address-fields"]')?.contains(b.element)
			);
		await remove?.trigger('click');
		expect(inAddresses(wrapper, SelectField)).toHaveLength(1);
		wrapper.unmount();
	});

	test('la etiqueta y los siete campos llegan al parche al guardar', async () => {
		const wrapper = form();
		await wrapper.findAllComponents(TextInput)[0].setValue('Ana Pérez');
		await button(wrapper, 'form.addAddress')?.trigger('click');

		inAddresses(wrapper, SelectField)[0].vm.$emit('update:modelValue', 'work');
		const values: Record<string, string> = {
			'form.address.street': 'Calle Falsa 123',
			'form.address.extended': 'piso 3',
			'form.address.poBox': '12',
			'form.address.city': 'Rosario',
			'form.address.region': 'Santa Fe',
			'form.address.postalCode': '2000',
			'form.address.country': 'Argentina',
		};
		for (const input of inAddresses(wrapper, TextInput)) {
			await input.setValue(values[placeholderOf(input)]);
		}

		const patch = await savedPatch(wrapper, 'form.create');
		expect(patch.addresses).toEqual([
			{
				label: 'work',
				po_box: '12',
				extended: 'piso 3',
				street: 'Calle Falsa 123',
				city: 'Rosario',
				region: 'Santa Fe',
				postal_code: '2000',
				country: 'Argentina',
			},
		]);
		wrapper.unmount();
	});

	test('al editar, precarga las direcciones del contacto', async () => {
		const wrapper = form({
			contact: emptyContact({
				name: 'Ana',
				account_id: 'a1',
				address_book_id: '1',
				store_id: '7',
				addresses: [ADDRESS],
			}),
		});
		const street = inAddresses(wrapper, TextInput).find(
			(i) => placeholderOf(i) === 'form.address.street'
		);
		expect((street?.props() as Record<string, unknown>).modelValue).toBe('Calle 1');
		wrapper.unmount();
	});

	test('una dirección con etiqueta propia se conserva al guardar sin tocarla', async () => {
		const wrapper = form({
			contact: emptyContact({
				name: 'Ana',
				account_id: 'a1',
				address_book_id: '1',
				store_id: '7',
				addresses: [{ ...ADDRESS, label: 'x-casa-de-verano' }],
			}),
		});
		const patch = await savedPatch(wrapper, 'form.save');
		expect(patch.addresses).toEqual([
			expect.objectContaining({ label: 'x-casa-de-verano', street: 'Calle 1', city: 'Rosario' }),
		]);
		wrapper.unmount();
	});
});
