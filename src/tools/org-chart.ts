import type { Contact } from './address-book';
import { normalizeUid } from './relations';

/**
 * Un renglón del organigrama: un contacto, cómo cuelga de su padre y a qué
 * profundidad, para dibujar el árbol como una lista con sangría.
 */
export interface OrgNode {
	contact: Contact;
	/**
	 * `root`: la cima del árbol. `report`: lo manda su padre (una relación
	 * `manager` hacia el padre). `assistant`: asiste a su padre.
	 */
	relation: 'root' | 'report' | 'assistant';
	/** Profundidad en el árbol: 0 es la raíz. La sangría sale de acá. */
	depth: number;
	/** El contacto sobre cuya ficha se dibuja el organigrama, para resaltarlo. */
	current: boolean;
}

/** ¿`node` cuelga de `ancestor`? Subiendo por los padres desde `node`. */
function isDescendant(node: string, ancestor: string, parentOf: Map<string, string>): boolean {
	let cur: string | undefined = node;
	const seen = new Set<string>();
	while (cur !== undefined) {
		if (cur === ancestor) return true;
		if (seen.has(cur)) return false;
		seen.add(cur);
		cur = parentOf.get(cur);
	}
	return false;
}

/**
 * El organigrama alrededor de un contacto: el árbol `manager`/`assistant` de su
 * organización, en preorden y con la profundidad de cada uno.
 *
 * Una relación `manager` hacia B pone a B arriba (B manda a quien la declara);
 * una `assistant` hacia C pone a C abajo (C asiste a quien la declara). Las dos
 * se reducen a una arista padre→hijo, y con eso se arma el bosque.
 *
 * **Sólo dentro de la organización** (`organization`, sin distinguir
 * mayúsculas): un organigrama es de una empresa, y una relación hacia alguien de
 * otra —o hacia un nombre suelto, sin `target_uid` que resolver— no entra. Un
 * contacto sin organización no tiene organigrama.
 *
 * Se devuelve sólo el árbol que contiene a este contacto, desde su raíz. Si el
 * contacto no tiene ningún vínculo `manager`/`assistant` resoluble, no hay árbol
 * y la lista va vacía.
 *
 * shortcut: el alcance es la cadena `organization` igual; un jefe cuya tarjeta
 * escribe la empresa distinto queda afuera. Upgrade si aparece ruido de datos:
 * normalizar la organización como se normaliza el uid.
 */
export function orgChartFor(contact: Contact, allContacts: Contact[]): OrgNode[] {
	const org = contact.organization.trim().toLowerCase();
	if (!org) return [];
	const scope = allContacts.filter((c) => c.organization.trim().toLowerCase() === org);

	const byUid = new Map<string, Contact>();
	for (const c of scope) {
		const key = normalizeUid(c.uid);
		if (key && !byUid.has(key)) byUid.set(key, c);
	}

	// Un solo padre por nodo (gana el primero, determinista) y sin cerrar
	// ciclos: así el grafo es siempre un bosque y recorrerlo termina.
	const parentOf = new Map<string, string>();
	const relationOf = new Map<string, 'report' | 'assistant'>();
	const childrenOf = new Map<string, string[]>();
	const addEdge = (parent: string, child: string, relation: 'report' | 'assistant') => {
		if (parent === child || parentOf.has(child) || isDescendant(parent, child, parentOf)) return;
		parentOf.set(child, parent);
		relationOf.set(child, relation);
		const kids = childrenOf.get(parent) ?? [];
		kids.push(child);
		childrenOf.set(parent, kids);
	};

	for (const c of scope) {
		const cu = normalizeUid(c.uid);
		if (!cu) continue;
		for (const r of c.related) {
			const tu = normalizeUid(r.target_uid);
			if (!tu || tu === cu || !byUid.has(tu)) continue;
			if (r.kind === 'manager') addEdge(tu, cu, 'report');
			else if (r.kind === 'assistant') addEdge(cu, tu, 'assistant');
		}
	}

	const cu = normalizeUid(contact.uid);
	if (!cu || (!parentOf.has(cu) && !childrenOf.has(cu))) return [];

	let root = cu;
	const climbed = new Set<string>([root]);
	while (parentOf.has(root)) {
		const up = parentOf.get(root) as string;
		if (climbed.has(up)) break;
		climbed.add(up);
		root = up;
	}

	const out: OrgNode[] = [];
	const visited = new Set<string>();
	const visit = (uid: string, depth: number, relation: OrgNode['relation']) => {
		const c = byUid.get(uid);
		if (!c || visited.has(uid)) return;
		visited.add(uid);
		out.push({ contact: c, relation, depth, current: uid === cu });
		for (const child of childrenOf.get(uid) ?? []) {
			visit(child, depth + 1, relationOf.get(child) ?? 'report');
		}
	};
	visit(root, 0, 'root');
	return out;
}
