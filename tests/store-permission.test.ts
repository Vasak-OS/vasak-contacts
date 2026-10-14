/**
 * Las marcas del permiso del almacén que el programa de Rust le pasa a la
 * ventana (`src/tools/store-permission.ts`). El texto lo pone la ventana en el
 * idioma de la sesión; acá se prueba que cada marca caiga donde tiene que caer,
 * y que una desconocida no se escape como mensaje crudo.
 */

import { describe, expect, test } from 'bun:test';
import {
	isStorePermissionDenied,
	REASK_DENIED,
	REASK_FAILED,
	REASK_LIMIT,
	reaskNoticeKey,
	STORE_PERMISSION_DENIED,
} from '@/tools/store-permission';

describe('reconocer el «no» del almacén', () => {
	test('la marca exacta es un «no»', () => {
		expect(isStorePermissionDenied(STORE_PERMISSION_DENIED)).toBe(true);
	});

	test('cualquier otro error no lo es', () => {
		expect(isStorePermissionDenied('el servicio no está')).toBe(false);
		expect(isStorePermissionDenied(new Error(STORE_PERMISSION_DENIED))).toBe(false);
		expect(isStorePermissionDenied(undefined)).toBe(false);
		expect(isStorePermissionDenied(null)).toBe(false);
	});
});

describe('traducir el error de volver a pedir', () => {
	test('cada marca tiene su clave', () => {
		expect(reaskNoticeKey(REASK_LIMIT)).toBe('permiso.limite');
		expect(reaskNoticeKey(REASK_DENIED)).toBe('permiso.denegado');
		expect(reaskNoticeKey(REASK_FAILED)).toBe('permiso.error');
	});

	test('una marca desconocida cae en el mensaje genérico, no en algo crudo', () => {
		expect(reaskNoticeKey('vaya-a-saber')).toBe('permiso.error');
		expect(reaskNoticeKey(undefined)).toBe('permiso.error');
	});
});
