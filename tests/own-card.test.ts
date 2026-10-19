/**
 * La tarjeta propia, «este soy yo» (vasak-contacts#10).
 *
 * La marca es una preferencia de la ventana, no un dato de la tarjeta, y la
 * propuesta sale de lo que dice el servidor: se prueba que se reconozca al
 * contacto correcto (la misma persona en dos cuentas no es la misma tarjeta) y
 * que un servidor que falla o no contesta deje elegir a mano, sin romper nada.
 */

import { afterEach, beforeEach, describe, expect, spyOn, test } from 'bun:test';
import { useOwnCard } from '@/composables/use-own-card';
import { type Contact, emptyContact } from '@/tools/address-book';
import {
	findOwnCard,
	loadOwnCardKey,
	ownCardKey,
	proposedOwnCard,
	saveOwnCardKey,
} from '@/tools/own-card';
import { answerCommand, invokedCommands, olvidarTodo } from './dobles';

const STORAGE_KEY = 'vasak-contacts.own-card';

const ANA = emptyContact({
	uid: 'uid-ana',
	name: 'Ana',
	account_id: 'a1',
	url: 'store:a1:/dav/book/ana.vcf',
});
const ANA_EN_OTRA_CUENTA = emptyContact({
	uid: 'uid-ana',
	name: 'Ana (otra cuenta)',
	account_id: 'a2',
	url: 'store:a2:/dav/book/ana.vcf',
});
const BRUNO = emptyContact({
	uid: 'uid-bruno',
	name: 'Bruno',
	account_id: 'a1',
	url: 'store:a1:/dav/book/bruno.vcf',
});

beforeEach(() => {
	globalThis.localStorage?.clear();
});
afterEach(() => {
	globalThis.localStorage?.clear();
	olvidarTodo();
});

describe('cómo se reconoce la tarjeta marcada', () => {
	test('por su identidad, o por el UID si no la tiene', () => {
		expect(ownCardKey(ANA)).toBe('store:a1:/dav/book/ana.vcf');
		expect(ownCardKey(emptyContact({ uid: 'solo-uid' }))).toBe('solo-uid');
	});

	test('la misma persona en dos cuentas son dos tarjetas distintas', () => {
		expect(ownCardKey(ANA)).not.toBe(ownCardKey(ANA_EN_OTRA_CUENTA));
		expect(findOwnCard([ANA_EN_OTRA_CUENTA, ANA], ownCardKey(ANA))).toBe(ANA);
	});

	test('sin marca, o con una tarjeta que ya no está, no hay tarjeta propia', () => {
		expect(findOwnCard([ANA, BRUNO], '')).toBeNull();
		expect(findOwnCard([BRUNO], ownCardKey(ANA))).toBeNull();
	});
});

describe('recordar la marca', () => {
	test('se guarda, se lee y se borra', () => {
		saveOwnCardKey('store:a1:/x');
		expect(loadOwnCardKey()).toBe('store:a1:/x');

		saveOwnCardKey('');
		expect(loadOwnCardKey()).toBe('');
		expect(globalThis.localStorage?.getItem(STORAGE_KEY)).toBeNull();
	});

	test('sin nada guardado no hay marca', () => {
		expect(loadOwnCardKey()).toBe('');
	});

	test('si el navegador no deja guardar ni leer, la ventana sigue', () => {
		const original = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
		const roto = () => {
			throw new Error('bloqueado');
		};
		Object.defineProperty(globalThis, 'localStorage', {
			configurable: true,
			value: { getItem: roto, setItem: roto, removeItem: roto },
		});
		try {
			expect(() => saveOwnCardKey('k')).not.toThrow();
			expect(() => saveOwnCardKey('')).not.toThrow();
			expect(loadOwnCardKey()).toBe('');
		} finally {
			if (original) {
				Object.defineProperty(globalThis, 'localStorage', original);
			}
		}
	});
});

describe('la tarjeta que propone el servidor', () => {
	test('se encuentra por el camino, entero o relativo, de la cuenta que lo dijo', () => {
		const contacts = [ANA_EN_OTRA_CUENTA, BRUNO, ANA];

		expect(proposedOwnCard(contacts, 'a1', 'https://nube.ejemplo.com/dav/book/ana.vcf')).toBe(ANA);
		expect(proposedOwnCard(contacts, 'a1', '/dav/book/ana.vcf')).toBe(ANA);
		expect(proposedOwnCard(contacts, 'a2', '/dav/book/ana.vcf')).toBe(ANA_EN_OTRA_CUENTA);
	});

	test('compara el camino decodificado: el almacén y el servidor no escapan igual', () => {
		const contact = emptyContact({ account_id: 'a1', url: 'store:a1:/dav/book/mar%C3%ADa.vcf' });
		const otro = emptyContact({ account_id: 'a1', url: 'https://x/dav/book/maría.vcf' });
		// `%7E` y `~` son el mismo carácter: ninguno de los dos lados lo normaliza.
		const tilde = emptyContact({ account_id: 'a1', url: 'store:a1:/dav/book/%7Eana.vcf' });

		expect(proposedOwnCard([contact], 'a1', 'https://x/dav/book/maría.vcf')).toBe(contact);
		expect(proposedOwnCard([otro], 'a1', '/dav/book/mar%C3%ADa.vcf')).toBe(otro);
		expect(proposedOwnCard([tilde], 'a1', '/dav/book/~ana.vcf')).toBe(tilde);
	});

	test('una sub-tarjeta de un recurso con varias no es la propuesta', () => {
		const sub = emptyContact({ account_id: 'a1', url: 'store:a1:/dav/book/ana.vcf#1' });

		expect(proposedOwnCard([sub], 'a1', '/dav/book/ana.vcf')).toBeNull();
		expect(proposedOwnCard([sub, ANA], 'a1', '/dav/book/ana.vcf')).toBe(ANA);
	});

	test('si ninguna coincide, no hay propuesta', () => {
		expect(proposedOwnCard([ANA, BRUNO], 'a1', '/dav/book/otra.vcf')).toBeNull();
		expect(proposedOwnCard([ANA], 'a3', '/dav/book/ana.vcf')).toBeNull();
		expect(proposedOwnCard([], 'a1', '/dav/book/ana.vcf')).toBeNull();
	});

	test('una dirección con una codificación rota se compara tal cual, sin romper', () => {
		const rota = emptyContact({ account_id: 'a1', url: 'store:a1:/dav/100%.vcf' });

		expect(proposedOwnCard([rota], 'a1', '/dav/100%.vcf')).toBe(rota);
		expect(proposedOwnCard([ANA], 'a1', '/dav/100%.vcf')).toBeNull();
	});
});

describe('el composable de la tarjeta propia', () => {
	function own(contacts: Contact[], accounts: string[]) {
		return useOwnCard(
			() => contacts,
			() => accounts
		);
	}

	test('marcar recuerda la tarjeta, y otra ventana la encuentra al abrir', () => {
		const first = own([ANA, BRUNO], ['a1']);
		expect(first.ownCard.value).toBeNull();

		first.mark(ANA);

		expect(first.isOwn(ANA)).toBe(true);
		expect(first.isOwn(BRUNO)).toBe(false);
		expect(first.ownCard.value).toBe(ANA);
		expect(own([ANA, BRUNO], ['a1']).ownCard.value).toBe(ANA);
	});

	test('«este soy yo» alterna, y marcar otra pasa la marca a la otra', () => {
		const card = own([ANA, BRUNO], ['a1']);

		card.toggle(ANA);
		expect(card.isOwn(ANA)).toBe(true);

		card.toggle(BRUNO);
		expect(card.isOwn(ANA)).toBe(false);
		expect(card.isOwn(BRUNO)).toBe(true);

		card.toggle(BRUNO);
		expect(card.ownCard.value).toBeNull();
		expect(loadOwnCardKey()).toBe('');
	});

	test('sin contacto, o sin marca, nadie es la tarjeta propia', () => {
		const card = own([ANA], ['a1']);

		expect(card.isOwn(null)).toBe(false);
		expect(card.isOwn(ANA)).toBe(false);
	});

	test('propone la tarjeta que el servidor dice, preguntando por cada cuenta', async () => {
		answerCommand('own_card_address', (args) =>
			args?.accountId === 'a2' ? '/dav/book/ana.vcf' : null
		);

		const found = await own([ANA, ANA_EN_OTRA_CUENTA], ['a1', 'a2']).propose();

		expect(found).toBe(ANA_EN_OTRA_CUENTA);
		expect(invokedCommands.map((c) => c.args)).toEqual([{ accountId: 'a1' }, { accountId: 'a2' }]);
	});

	test('una cuenta que falla no impide probar la siguiente', async () => {
		const log = spyOn(console, 'error').mockImplementation(() => {});
		answerCommand('own_card_address', (args) => {
			if (args?.accountId === 'a1') {
				throw new Error('sin permiso');
			}
			return '/dav/book/ana.vcf';
		});
		try {
			const found = await own([ANA, ANA_EN_OTRA_CUENTA], ['a1', 'a2']).propose();

			expect(found).toBe(ANA_EN_OTRA_CUENTA);
			expect(log).toHaveBeenCalledTimes(1);
		} finally {
			log.mockRestore();
		}
	});

	test('una dirección que no es de ningún contacto no es una propuesta', async () => {
		answerCommand('own_card_address', () => '/dav/book/desconocida.vcf');

		expect(await own([ANA, BRUNO], ['a1']).propose()).toBeNull();
	});

	test('si ninguna cuenta contesta, la persona elige a mano', async () => {
		expect(await own([ANA], ['a1']).propose()).toBeNull();
		expect(await own([ANA], []).propose()).toBeNull();
	});
});
