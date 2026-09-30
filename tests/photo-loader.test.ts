import { describe, expect, test } from 'bun:test';
import { createPhotoLoader, type Invoke } from '../src/tools/photo-loader';

const PNG = 'data:image/png;base64,iVBORw0KGgo=';

/** Un `invoke` de mentira que anota lo que se le pidió. */
function fakeInvoke(answer: (url: string) => unknown) {
	const calls: { command: string; args: Record<string, unknown> | undefined }[] = [];
	const invoke = (async (command: string, args?: Record<string, unknown>) => {
		calls.push({ command, args });
		const result = answer(String(args?.url ?? ''));
		if (result instanceof Error) throw result;
		return result;
	}) as Invoke;
	return { invoke, calls };
}

describe('createPhotoLoader', () => {
	test('la foto se le pide al programa, no a la dirección', async () => {
		const { invoke, calls } = fakeInvoke(() => PNG);
		const load = createPhotoLoader(invoke);

		expect(await load('https://lh3.googleusercontent.com/a')).toBe(PNG);
		expect(calls).toEqual([
			{ command: 'contact_photo', args: { url: 'https://lh3.googleusercontent.com/a' } },
		]);
	});

	test('una vez por sesión, abra la ficha las veces que la abra', async () => {
		const { invoke, calls } = fakeInvoke(() => PNG);
		const load = createPhotoLoader(invoke);

		await load('https://x/a');
		await load('https://x/a');
		// Y dos fichas a la vez esperan el mismo pedido.
		await Promise.all([load('https://x/b'), load('https://x/b')]);
		expect(calls.map((c) => c.args?.url)).toEqual(['https://x/a', 'https://x/b']);
	});

	test('lo que falla se vuelve a preguntar la próxima vez', async () => {
		// El programa contesta desde su marca de fallo sin salir a la red:
		// memorizarlo acá haría que una foto que se cayó un momento no
		// apareciera hasta cerrar la ventana.
		let answer: unknown = null;
		const { invoke, calls } = fakeInvoke(() => answer);
		const load = createPhotoLoader(invoke);

		expect(await load('https://x/a')).toBe('');
		answer = PNG;
		expect(await load('https://x/a')).toBe(PNG);
		expect(calls).toHaveLength(2);
	});

	test('un error del programa se ve como que no hay foto', async () => {
		const { invoke } = fakeInvoke(() => new Error('sin servicio'));
		expect(await createPhotoLoader(invoke)('https://x/a')).toBe('');
	});

	test('sólo una imagen embebida llega al src', async () => {
		// Si el programa contestara otra cosa —una dirección, un
		// `javascript:`—, no va a parar a un `<img>`.
		for (const wrong of ['https://x/a.png', 'javascript:alert(1)', 'data:text/html,<b>', 42, {}]) {
			const { invoke } = fakeInvoke(() => wrong);
			expect(await createPhotoLoader(invoke)('https://x/a')).toBe('');
		}
	});

	test('sin dirección no se pregunta nada', async () => {
		const { invoke, calls } = fakeInvoke(() => PNG);
		expect(await createPhotoLoader(invoke)('')).toBe('');
		expect(calls).toHaveLength(0);
	});
});
