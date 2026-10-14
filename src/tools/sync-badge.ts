/**
 * La insignia «sin sincronizar» de un contacto, a partir de su `sync_state`.
 *
 * Aparte de los componentes para que la lista y la ficha muestren lo mismo: un
 * contacto `pending` y uno `conflict` no se ven igual, y decidir el tono en cada
 * lado por separado es cómo terminan distintos. Un contacto `synced` no lleva
 * insignia —no hay nada que avisar—, así que devuelve `null`.
 *
 * El tono sale del esquema (lo pinta `Badge` de `vue-libvasak`): `warning` para
 * lo que espera subir, `error` para lo que chocó y hay que resolver. La clave de
 * texto la traduce la ventana.
 */

import type { SyncState } from '@/tools/address-book';

/** Qué insignia corresponde a un estado, o `null` si no lleva ninguna. */
export interface SyncBadge {
	tone: 'warning' | 'error';
	/** La clave de traducción del texto corto de la insignia. */
	key: string;
	/** La clave del texto entero, para el globo del sistema. */
	titleKey: string;
}

/** La insignia de un estado de sincronización, o `null` si está al día. */
export function syncBadge(state: SyncState): SyncBadge | null {
	switch (state) {
		case 'pending':
			return { tone: 'warning', key: 'sync.badge.pending', titleKey: 'sync.badge.pendingTitle' };
		case 'conflict':
			return { tone: 'error', key: 'sync.badge.conflict', titleKey: 'sync.badge.conflictTitle' };
		default:
			return null;
	}
}
