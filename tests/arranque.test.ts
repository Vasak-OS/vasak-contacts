import { describe, expect, mock, test } from 'bun:test';
import { esperarArranque } from '../src/tools/arranque';

/** Una promesa que termina sola dentro de `ms`. */
const enMs = (ms: number) => new Promise((listo) => setTimeout(listo, ms));

describe('esperarArranque', () => {
	test('espera a que terminen todas y avisa que llegó', async () => {
		const terminadas: string[] = [];
		const tarea = (nombre: string, ms: number) =>
			enMs(ms).then(() => {
				terminadas.push(nombre);
			});

		expect(await esperarArranque([tarea('textos', 5), tarea('tema', 20)], 1000)).toBe(true);
		// Las dos, no la primera: si sólo se esperara a una, la ventana se abriría
		// con los textos puestos y el tema todavía sin aplicar.
		expect(terminadas).toEqual(['textos', 'tema']);
	});

	test('una tarea que falla no impide abrir la ventana', async () => {
		// Es la decisión de fondo: sin traducciones se ven las claves y sin
		// configuración se ve el tema por omisión, y las dos cosas son usables.
		// Lo que no es usable es que la aplicación no abra.
		const rota = Promise.reject(new Error('el backend dijo que no'));
		expect(await esperarArranque([rota, enMs(1)], 1000)).toBe(true);
	});

	test('un rechazo no queda sin atender', async () => {
		// Sin el `catch` de cada tarea, esto deja un «unhandled rejection» que en
		// el WebView se ve como un error suelto en la consola sin contexto.
		const sinAtender: unknown[] = [];
		const anotar = (e: PromiseRejectionEvent | unknown) => sinAtender.push(e);
		process.on('unhandledRejection', anotar);

		await esperarArranque([Promise.reject(new Error('x'))], 1000);
		await enMs(20);

		process.off('unhandledRejection', anotar);
		expect(sinAtender).toEqual([]);
	});

	test('vencido el plazo, devuelve false y no espera más', async () => {
		const PLAZO = 30;
		const nunca = new Promise(() => {});
		const antes = Date.now();
		expect(await esperarArranque([nunca], PLAZO)).toBe(false);
		const tardo = Date.now() - antes;

		// Que devuelva `false` **y vuelva**: si se quedara esperando igual, la
		// ventana no abriría nunca, que es el caso que el plazo existe para evitar.
		//
		// La tolerancia va atada al plazo y no fija: con un techo de un segundo
		// para un plazo de 30 ms, una regresión que lo retrasara cientos de
		// milisegundos pasaba igual, y entonces la prueba no comprueba el plazo
		// sino que el programa no se cuelga. El margen es para la máquina de
		// integración, que no es puntual.
		expect(tardo).toBeGreaterThanOrEqual(PLAZO - 5);
		expect(tardo).toBeLessThan(PLAZO * 10);
	});

	test('sin tareas no espera nada', async () => {
		expect(await esperarArranque([], 1000)).toBe(true);
	});

	test('el temporizador no queda vivo después de terminar', async () => {
		// Un plazo de segundos deja un `setTimeout` colgado mucho después de que
		// la ventana ya se dibujó.
		const limpiados: unknown[] = [];
		const original = globalThis.clearTimeout;
		const espia = mock((id: unknown) => {
			limpiados.push(id);
			return original(id as Parameters<typeof original>[0]);
		});
		globalThis.clearTimeout = espia as unknown as typeof clearTimeout;

		try {
			await esperarArranque([enMs(1)], 60_000);
		} finally {
			globalThis.clearTimeout = original;
		}

		expect(limpiados.length).toBe(1);
	});
});
