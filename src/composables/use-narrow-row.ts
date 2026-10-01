import { onScopeDispose, ref, watch } from 'vue';
import { narrowRowPx } from '@/tools/narrow-layout';

/**
 * Si la fila de la ventana está por debajo de 36rem, para lo que no se puede
 * decidir con una consulta de contenedor: la barra no está adentro de la fila,
 * así que no la puede consultar, y el buscador se pliega en una lupa con una
 * propiedad, no con una clase.
 *
 * Con un `ResizeObserver` sobre la fila misma, y no con `resize` ni
 * `matchMedia`: en WebKitGTK ninguno de los dos avisa. Es la misma fila y el
 * mismo umbral que miran las clases de `tools/narrow-layout.ts`, así que las dos
 * mitades cambian juntas.
 */
export function useNarrowRow(row: () => HTMLElement | null | undefined) {
	const narrow = ref(false);
	let observer: ResizeObserver | null = null;

	function measure(width: number) {
		// Sin ancho —el primer cuadro, una fila oculta, una prueba sin diseño—
		// no se sabe nada: queda lo que había, y al principio es ancha, que es
		// como se veía siempre.
		if (width <= 0) return;
		narrow.value = width < narrowRowPx();
	}

	watch(
		row,
		(element) => {
			observer?.disconnect();
			observer = null;
			if (!element || typeof ResizeObserver === 'undefined') return;
			observer = new ResizeObserver((entries) => {
				const entry = entries.at(-1);
				if (entry) measure(entry.contentRect.width);
			});
			observer.observe(element);
			measure(element.getBoundingClientRect().width);
		},
		{ immediate: true }
	);

	onScopeDispose(() => observer?.disconnect());

	return { narrow, measure };
}
