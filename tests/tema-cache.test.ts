import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { aplicarTema, CLAVE_TEMA, guardarTema } from '../src/tools/tema-cache';

/** Un `Storage` de prueba: no toca el `localStorage` global. */
function almacenDePrueba(inicial: Record<string, string> = {}): Storage {
	const datos = new Map(Object.entries(inicial));
	return {
		getItem: (clave: string) => datos.get(clave) ?? null,
		setItem: (clave: string, valor: string) => void datos.set(clave, valor),
	} as unknown as Storage;
}

/** Un almacén inaccesible, como `localStorage` en modo privado o bloqueado. */
function almacenQueTira(): Storage {
	const tirar = () => {
		throw new Error('SecurityError');
	};
	return { getItem: tirar, setItem: tirar } as unknown as Storage;
}

const nuevaRaiz = () => document.createElement('div');

describe('guardarTema', () => {
	test('guarda la clase oscura y sólo las variables `--*` del estilo en línea', () => {
		const raiz = nuevaRaiz();
		raiz.classList.add('dark');
		raiz.style.setProperty('--foo', 'red');
		raiz.style.setProperty('--ui-background', '#101010');
		raiz.style.color = 'blue'; // una propiedad común no es del esquema
		const almacen = almacenDePrueba();

		guardarTema(raiz, almacen);

		expect(JSON.parse(almacen.getItem(CLAVE_TEMA) as string)).toEqual({
			dark: true,
			vars: { '--foo': 'red', '--ui-background': '#101010' },
		});
	});

	test('sin la clase `.dark` guarda dark en falso', () => {
		const raiz = nuevaRaiz();
		raiz.style.setProperty('--foo', 'red');
		const almacen = almacenDePrueba();

		guardarTema(raiz, almacen);

		expect(JSON.parse(almacen.getItem(CLAVE_TEMA) as string).dark).toBe(false);
	});

	test('la clave es la que lee el script de index.html', () => {
		expect(CLAVE_TEMA).toBe('vsk-theme');
	});

	test('un almacén inaccesible no rompe el arranque', () => {
		expect(() => guardarTema(nuevaRaiz(), almacenQueTira())).not.toThrow();
	});
});

describe('aplicarTema', () => {
	test('reaplica la clase oscura y las variables recordadas', () => {
		const almacen = almacenDePrueba({
			[CLAVE_TEMA]: JSON.stringify({ dark: true, vars: { '--foo': 'red', '--ui-background': '#101010' } }),
		});
		const raiz = nuevaRaiz();

		aplicarTema(raiz, almacen);

		expect(raiz.classList.contains('dark')).toBe(true);
		expect(raiz.style.getPropertyValue('--foo')).toBe('red');
		expect(raiz.style.getPropertyValue('--ui-background')).toBe('#101010');
	});

	test('con dark en falso no agrega la clase `.dark`', () => {
		const almacen = almacenDePrueba({
			[CLAVE_TEMA]: JSON.stringify({ dark: false, vars: { '--foo': 'red' } }),
		});
		const raiz = nuevaRaiz();

		aplicarTema(raiz, almacen);

		expect(raiz.classList.contains('dark')).toBe(false);
		expect(raiz.style.getPropertyValue('--foo')).toBe('red');
	});

	test('sin nada guardado no toca la raíz ni lanza', () => {
		const raiz = nuevaRaiz();

		expect(() => aplicarTema(raiz, almacenDePrueba())).not.toThrow();

		expect(raiz.classList.contains('dark')).toBe(false);
		expect(raiz.style.length).toBe(0);
	});

	test('un JSON inválido no lanza ni toca la raíz', () => {
		const raiz = nuevaRaiz();

		expect(() => aplicarTema(raiz, almacenDePrueba({ [CLAVE_TEMA]: '{no es json' }))).not.toThrow();

		expect(raiz.style.length).toBe(0);
	});

	test('un almacén inaccesible no rompe el arranque', () => {
		expect(() => aplicarTema(nuevaRaiz(), almacenQueTira())).not.toThrow();
	});
});

describe('ida y vuelta', () => {
	test('aplicar lo que dejó guardar reproduce el tema', () => {
		const origen = nuevaRaiz();
		origen.classList.add('dark');
		origen.style.setProperty('--foo', 'red');
		origen.style.setProperty('--ui-background', '#101010');
		const almacen = almacenDePrueba();
		guardarTema(origen, almacen);

		const destino = nuevaRaiz();
		aplicarTema(destino, almacen);

		expect(destino.classList.contains('dark')).toBe(true);
		expect(destino.style.getPropertyValue('--foo')).toBe('red');
		expect(destino.style.getPropertyValue('--ui-background')).toBe('#101010');
	});
});

/**
 * El cableado: el módulo solo no evita el parpadeo, hace falta que el script
 * pre-boot de `index.html` lo reaplique antes de montar y que `main.ts` (primera
 * lectura) y `App.vue` (cambios) guarden el tema una vez cargada la configuración.
 */
describe('cableado contra el parpadeo de tema', () => {
	const leer = (ruta: string) => readFileSync(fileURLToPath(new URL(ruta, import.meta.url)), 'utf8');

	test('index.html reaplica el tema guardado en el <head>, antes del bundle', () => {
		const html = leer('../index.html');
		const script = html.match(/<script>([\s\S]*?)<\/script>/);
		expect(script).not.toBeNull();
		expect(html.indexOf('<script>')).toBeLessThan(html.indexOf('</head>'));
		expect(html.indexOf('<script>')).toBeLessThan(html.indexOf('type="module"'));

		const cuerpo = (script as RegExpMatchArray)[1];
		expect(cuerpo).toContain(`"${CLAVE_TEMA}"`);
		expect(cuerpo).toContain('classList.add("dark")');
		expect(cuerpo).toContain('setProperty');
	});

	test('el script de index.html, ejecutado, reproduce lo que guardó guardarTema', () => {
		const html = leer('../index.html');
		const cuerpo = (html.match(/<script>([\s\S]*?)<\/script>/) as RegExpMatchArray)[1];
		const raiz = document.documentElement;
		const antes = localStorage.getItem(CLAVE_TEMA);
		try {
			localStorage.setItem(
				CLAVE_TEMA,
				JSON.stringify({ dark: true, vars: { '--tema-prueba': 'rebeccapurple' } })
			);

			new Function(cuerpo)();

			expect(raiz.classList.contains('dark')).toBe(true);
			expect(raiz.style.getPropertyValue('--tema-prueba')).toBe('rebeccapurple');
		} finally {
			raiz.classList.remove('dark');
			raiz.style.removeProperty('--tema-prueba');
			if (antes === null) localStorage.removeItem(CLAVE_TEMA);
			else localStorage.setItem(CLAVE_TEMA, antes);
		}
	});

	test('main.ts guarda el tema al leer la configuración, antes de montar', () => {
		const ts = leer('../src/main.ts');

		expect(ts).toContain("from '@/tools/tema-cache'");
		expect(ts).toMatch(/loadConfig\(\)\s*\.then\(\(\) => guardarTema\(\)\)/);
		expect(ts.indexOf('guardarTema()')).toBeLessThan(ts.indexOf("app.mount('#app')"));
	});

	test('App.vue guarda el tema al recargar la configuración por un cambio', () => {
		const vue = leer('../src/App.vue');

		expect(vue).toContain("from '@/tools/tema-cache'");
		expect(vue).toMatch(/loadConfig\(\)\.then\(\(\) => guardarTema\(\)\)/);
	});
});
