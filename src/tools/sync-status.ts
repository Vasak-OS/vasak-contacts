/**
 * Qué se dice de la sincronización de cada cuenta.
 *
 * El estado lo da el almacén del sistema (`GetStatus` de `AccountsStore`), y lo
 * que no se pudo pedir con «Actualizar» lo dice el programa (`request_sync`).
 * Acá sólo se decide **qué línea** va debajo de la cuenta: con qué tono, qué
 * texto y si late mientras trabaja.
 *
 * Lo que no está se ve **no disponible, nunca roto** (decisión 2 del taller): un
 * «Actualizar» que no hizo nada y no lo dice se lee como que no había nada
 * nuevo, y una agenda que no se sincroniza sin decirlo se lee como una agenda
 * al día.
 */

export type SyncTone = 'neutral' | 'accent' | 'success' | 'warning' | 'error';

/** El estado de los contactos de una cuenta en el almacén. */
export interface AccountSyncStatus {
	account_id: string;
	/** `off`, `pending`, `syncing`, `synced`, `unavailable` o `failed`. */
	state: string;
	detail: string;
	/** RFC 3339, o `null` si nunca terminó una vuelta. */
	last_synced_at: string | null;
}

/** Una cuenta a la que «Actualizar» no le pudo pedir una vuelta. */
export interface SyncRequestFailure {
	account_id: string;
	reason: 'denied' | 'too_soon' | 'unavailable' | 'failed';
}

/** La línea que va debajo de la cuenta. `key` es la clave de traducción. */
export interface SyncLine {
	tone: SyncTone;
	key: string;
	/** Lo que reemplaza `{0}` en el texto, o vacío. */
	arg: string;
	/** Si el punto late: mientras el sincronizador trabaja. */
	pulse: boolean;
}

/**
 * Cuándo fue la última vuelta buena, corto: la hora si fue hoy y el día si
 * no. Vacío si la fecha no se entiende.
 */
export function syncedWhen(iso: string, now: Date = new Date(), locale?: string): string {
	const when = new Date(iso);
	if (Number.isNaN(when.getTime())) return '';
	const sameDay =
		when.getFullYear() === now.getFullYear() &&
		when.getMonth() === now.getMonth() &&
		when.getDate() === now.getDate();
	return sameDay
		? when.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' })
		: when.toLocaleDateString(locale, { day: 'numeric', month: 'short' });
}

/**
 * La línea de una cuenta, o `null` si no hay nada que decir (el área todavía no
 * se encendió). Lo que no se pudo pedir va primero: es lo que la persona acaba
 * de hacer y lo que espera ver.
 */
export function syncLine(
	status: AccountSyncStatus | undefined,
	failure: SyncRequestFailure | undefined,
	now: Date = new Date(),
	locale?: string
): SyncLine | null {
	if (failure) {
		return {
			tone: failure.reason === 'too_soon' ? 'neutral' : 'warning',
			key: `sync.request.${failure.reason}`,
			arg: '',
			pulse: false,
		};
	}
	switch (status?.state) {
		case 'pending':
			return { tone: 'neutral', key: 'sync.pending', arg: '', pulse: true };
		case 'syncing':
			return { tone: 'accent', key: 'sync.syncing', arg: '', pulse: true };
		case 'synced': {
			const when = status.last_synced_at ? syncedWhen(status.last_synced_at, now, locale) : '';
			return when
				? { tone: 'success', key: 'sync.syncedAt', arg: when, pulse: false }
				: { tone: 'success', key: 'sync.synced', arg: '', pulse: false };
		}
		case 'failed':
			return { tone: 'error', key: 'sync.failed', arg: status.detail, pulse: false };
		case 'unavailable':
			return { tone: 'warning', key: 'sync.unavailable', arg: status.detail, pulse: false };
		default:
			return null;
	}
}
