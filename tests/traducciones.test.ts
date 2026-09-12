import { describe, expect, mock, test } from 'bun:test';

/**
 * El catálogo que el proceso de Rust devolvería, con una sola clave: lo que se
 * prueba acá no es el contenido sino **cuándo** la interfaz lo tiene.
 */
const CATALOGO = { es: { 'vsk.prueba': 'Traducido' } };

mock.module('@tauri-apps/api/core', () => ({
	invoke: async (comando: string) => {
		if (comando === 'plugin:i18n|load_translations') return CATALOGO;
		if (comando === 'plugin:i18n|get_locale') return 'es';
		return null;
	},
}));
mock.module('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const { default: I18n, useI18n } = await import('@vasakgroup/tauri-plugin-i18n');

/**
 * El destello de las claves crudas al arrancar, y por qué esperar no alcanzaba.
 *
 * El plugin tiene **dos** estados con el mismo catálogo: el de la clase `I18n`,
 * que vive en un campo del singleton, y el del composable `useI18n()`, que vive
 * en unas referencias de módulo aparte. El `t()` que usan los componentes lee el
 * segundo, y cargar el primero no lo toca.
 *
 * `main.ts` esperaba al primero. O sea que esperaba de verdad, tres segundos de
 * plazo y todo, a algo que no cambiaba nada de lo que se iba a dibujar: la
 * ventana se montaba con el composable vacío, su `t()` devolvía la clave tal
 * cual, y los textos aparecían recién cuando el `onMounted` del primer
 * componente cargaba todo otra vez por su cuenta.
 */
describe('cargar las traducciones antes de montar', () => {
	test('antes de cargar, el t() devuelve la clave', () => {
		expect(useI18n().t('vsk.prueba')).toBe('vsk.prueba');
	});

	test('cargar la clase deja listo el t() de los componentes', async () => {
		// Esto es lo que hace `main.ts` antes de montar, y es lo que tiene que
		// alcanzar. Hasta la 2.3.0 no alcanzaba: la llamada andaba, no fallaba, no
		// avisaba, y no servía para lo que se la usaba, porque el catálogo quedaba
		// en un estado que el `t()` de los componentes no lee.
		await I18n.getInstance().load();

		const { t, isLoaded, locale } = useI18n();
		expect(isLoaded.value).toBe(true);
		expect(locale.value).toBe('es');
		expect(t('vsk.prueba')).toBe('Traducido');
	});

	test('una clave que no está en el catálogo se sigue viendo cruda', async () => {
		// El respaldo del plugin no cambia: una clave sin traducir se ve, que es
		// lo que hace que alguien la agregue. Lo que no puede pasar es que se
		// vean **todas** porque el catálogo todavía no llegó.
		expect(useI18n().t('vsk.no.existe')).toBe('vsk.no.existe');
	});
});
