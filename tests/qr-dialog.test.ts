/**
 * El diálogo del código QR (vasak-contacts#10): las casillas y el código.
 *
 * Lo que se prueba es lo que la persona nota: que no pueda marcar más de lo que
 * entra, que destildar reabra el lugar, que el código que ve sea siempre el de
 * las casillas de ahora, y que un error del programa se diga en su idioma.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { ActionButton, Checkbox } from '@vasakgroup/vue-libvasak';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import QrDialog from '@/components/contacts/QrDialog.vue';
import { emptyContact } from '@/tools/address-book';
import { answerCommand, invokedCommands, olvidarTodo } from './dobles';

const SVG = '<svg viewBox="0 0 8 8"><rect width="8" height="8"/></svg>';

const ANA = emptyContact({
	name: 'Ana Pérez',
	organization: 'Vasak Group',
	phones: [
		{ label: '', value: '111' },
		{ label: '', value: '222' },
		{ label: '', value: '333' },
	],
	emails: [
		{ label: '', value: 'a@x.com' },
		{ label: '', value: 'b@x.com' },
	],
});

let wrapper: VueWrapper | null = null;

afterEach(() => {
	wrapper?.unmount();
	wrapper = null;
	olvidarTodo();
});

function dialog(contact = ANA) {
	wrapper = mount(QrDialog, {
		attachTo: document.body.appendChild(document.createElement('div')),
		props: { open: true, contact },
	});
	return wrapper;
}

/** La casilla con ese texto. */
function box(label: string) {
	const found = wrapper?.findAllComponents(Checkbox).find((c) => c.props('label') === label);
	if (!found) {
		throw new Error(`no hay casilla «${label}»`);
	}
	return found;
}

const generateButton = () =>
	wrapper?.findAllComponents(ActionButton).find((b) => b.props('label') === 'qr.generate');

const qrBox = () => document.body.querySelector('[data-testid="qr-code"]');

describe('las casillas', () => {
	test('al abrir vienen marcados el nombre, dos teléfonos, un correo y la organización', () => {
		dialog();

		expect(box('Ana Pérez').props('modelValue')).toBe(true);
		expect(box('Vasak Group').props('modelValue')).toBe(true);
		expect(box('111').props('modelValue')).toBe(true);
		expect(box('222').props('modelValue')).toBe(true);
		expect(box('333').props('modelValue')).toBe(false);
		expect(box('a@x.com').props('modelValue')).toBe(true);
		expect(box('b@x.com').props('modelValue')).toBe(false);
	});

	test('con dos teléfonos marcados, el tercero no se puede marcar; los marcados sí se pueden destildar', () => {
		dialog();

		expect(box('333').props('disabled')).toBe(true);
		expect(box('111').props('disabled')).toBe(false);
		expect(box('222').props('disabled')).toBe(false);
	});

	test('destildar un teléfono reabre el lugar', async () => {
		dialog();

		await box('111').vm.$emit('update:modelValue', false);

		expect(box('333').props('disabled')).toBe(false);
		await box('333').vm.$emit('update:modelValue', true);
		expect(box('333').props('modelValue')).toBe(true);
		// Ya vuelven a ser dos: el de antes quedó destildado y ahora el que no entra es el 111.
		expect(box('111').props('disabled')).toBe(true);
	});

	test('con un correo marcado, el otro no se puede marcar hasta destildar el primero', async () => {
		dialog();

		expect(box('b@x.com').props('disabled')).toBe(true);

		await box('a@x.com').vm.$emit('update:modelValue', false);
		expect(box('b@x.com').props('disabled')).toBe(false);
	});

	test('un contacto sin teléfono ni correo no muestra esas secciones', () => {
		dialog(emptyContact({ name: 'Solo Nombre' }));

		expect(wrapper?.findAllComponents(Checkbox).map((c) => c.props('label'))).toEqual(['Solo Nombre']);
	});
});

describe('generar el código', () => {
	test('manda lo marcado, y sólo eso, y dibuja lo que vuelve', async () => {
		answerCommand('generate_vcard_qr', () => SVG);
		dialog();
		await box('Vasak Group').vm.$emit('update:modelValue', false);
		await box('222').vm.$emit('update:modelValue', false);

		await generateButton()?.trigger('click');
		await flushPromises();

		expect(invokedCommands).toEqual([
			{
				command: 'generate_vcard_qr',
				args: { card: { name: 'Ana Pérez', phones: ['111'], email: 'a@x.com', organization: '' } },
			},
		]);
		expect(qrBox()?.innerHTML).toContain('<rect');
	});

	test('sin nada marcado no se puede generar', async () => {
		dialog(emptyContact({ name: 'Ana' }));
		await box('Ana').vm.$emit('update:modelValue', false);

		expect(generateButton()?.props('disabled')).toBe(true);
		await generateButton()?.trigger('click');
		expect(invokedCommands).toHaveLength(0);
	});

	test('tocar una casilla borra el código: lo que se ve es siempre lo marcado ahora', async () => {
		answerCommand('generate_vcard_qr', () => SVG);
		dialog();
		await generateButton()?.trigger('click');
		await flushPromises();
		expect(qrBox()).not.toBeNull();

		await box('222').vm.$emit('update:modelValue', false);

		expect(qrBox()).toBeNull();
		expect(document.body.querySelector('[data-testid="qr-placeholder"]')).not.toBeNull();
	});

	test('un error del programa se dice con su texto traducido y no deja un código viejo', async () => {
		answerCommand('generate_vcard_qr', () => SVG);
		dialog();
		await generateButton()?.trigger('click');
		await flushPromises();
		expect(qrBox()).not.toBeNull();

		answerCommand('generate_vcard_qr', () => {
			throw 'qr-too-long';
		});
		await generateButton()?.trigger('click');
		await flushPromises();

		expect(qrBox()).toBeNull();
		expect(document.body.querySelector('[role="alert"]')?.textContent).toContain('qr.tooLong');
	});

	test('un error desconocido cae en el mensaje genérico', async () => {
		answerCommand('generate_vcard_qr', () => {
			throw new Error('algo del sistema');
		});
		dialog();
		await generateButton()?.trigger('click');
		await flushPromises();

		expect(document.body.querySelector('[role="alert"]')?.textContent).toContain('qr.error');
	});

	test('al abrir con otro contacto se arma de nuevo y no queda el código del anterior', async () => {
		answerCommand('generate_vcard_qr', () => SVG);
		dialog();
		await generateButton()?.trigger('click');
		await flushPromises();
		expect(qrBox()).not.toBeNull();

		await wrapper?.setProps({ contact: emptyContact({ name: 'Bruno' }) });

		expect(qrBox()).toBeNull();
		expect(box('Bruno').props('modelValue')).toBe(true);
	});
});
