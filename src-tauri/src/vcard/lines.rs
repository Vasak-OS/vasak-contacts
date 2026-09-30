//! Las líneas y las propiedades de una tarjeta: juntar lo plegado, separar
//! nombre, parámetros y valor, y deshacer las codificaciones.

// ---------------------------------------------------------------------------
// Las líneas
// ---------------------------------------------------------------------------

/// Junta las líneas partidas de una tarjeta.
///
/// **Hay dos formas de partir una línea y no se parecen en nada.**
///
/// La normal, de la 3.0 y la 4.0: se corta a 75 octetos y la siguiente empieza
/// con un espacio o una tabulación. Sin volver a juntarlas, un nombre largo
/// aparece cortado y una foto en base64 —que ocupa cientos de líneas— se
/// interpreta como cientos de propiedades basura.
///
/// La otra es de la 2.1, y sólo dentro de un valor en `quoted-printable`: la
/// línea termina en `=` y la siguiente **no lleva nada adelante**. Sin
/// reconocerla, la línea que sigue no tiene dos puntos y se descarta entera, y
/// el valor queda cortado con un signo de igual pegado al final — que es
/// exactamente el síntoma que leer la 2.1 viene a evitar.
///
/// Por eso este juntador tiene que saber de `quoted-printable`: no es
/// acoplamiento de más, es cómo está definido el formato. Y por eso el `=` no
/// junta líneas por sí solo — el base64 de una foto termina en `=` y se comería
/// la propiedad siguiente.
/// **Si una línea es `quoted-printable` se decide una sola vez**, cuando aparece
/// su primer dos puntos: lo de antes no cambia por más que se le peguen
/// continuaciones. Mirarlo de nuevo en cada una hacía que una tarjeta de medio
/// mega —los parámetros largos y miles de continuaciones— tardara **casi medio
/// minuto**: `last` crece, y `quedo_a_medias(&last)` volvía a pasar a
/// minúsculas todo lo acumulado. Un solo `to_ascii_lowercase` de la mitad de
/// una tarjeta, repetido por cada una de sus miles de líneas, es un tiempo que
/// crece como el cuadrado del tamaño.
pub fn unfold_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    // De la última línea: hasta dónde se buscaron los dos puntos, y si ya
    // aparecieron, si el valor va en `quoted-printable`.
    let mut scanned = 0;
    let mut quoted_printable: Option<bool> = None;
    // Si al terminar el fragmento anterior había una comilla abierta: un
    // parámetro partido a la mitad puede abrirla en uno y cerrarla en el otro.
    let mut quote_open = false;

    for raw_line in text.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

        // Continuación de la 2.1: se le saca el `=` que anunciaba que seguía y
        // se pega lo que vino, sin mirar con qué empieza. Que termine en `=` es
        // un `ends_with`, que es constante: acá está la linealidad.
        let qp_continues =
            quoted_printable == Some(true) && lines.last().is_some_and(|last| last.ends_with('='));

        if qp_continues {
            if let Some(last) = lines.last_mut() {
                last.pop();
                last.push_str(line);
                continue;
            }
        }

        match line.strip_prefix([' ', '\t']) {
            // Una **continuación de la 3.0 y la 4.0** —la línea partida a 75
            // octetos— no reinicia nada: es la misma línea lógica, y lo que se
            // decidió de sus primeros fragmentos sigue valiendo. Por eso
            // `scanned` existe, y por eso la búsqueda no se vuelve a hacer.
            Some(continuation) => match lines.last_mut() {
                Some(last) => last.push_str(continuation),
                // Sin línea anterior no hay a qué pegarse: esta es la primera.
                None => {
                    lines.push(continuation.to_string());
                    (scanned, quoted_printable, quote_open) = (0, None, false);
                }
            },
            // Una línea nueva **siempre** vuelve a empezar la búsqueda: lo que se
            // decidiera sobre la anterior no dice nada sobre esta. Sin esto, el
            // `Some(false)` de una línea que no era `quoted-printable` se corría a
            // la siguiente y la búsqueda ni se hacía: una 2.1 con una línea
            // cualquiera antes se leía con el `=` pegado al final.
            None => {
                lines.push(line.to_string());
                (scanned, quoted_printable, quote_open) = (0, None, false);
            }
        }

        // **Una vez que el separador se encontró, no se busca más.**
        //
        // `quoted_printable` es el estado de la línea *lógica*: cuando se decidió, se
        // decidió, y volver a mirar los parámetros no puede cambiarlo. Y mirar
        // los parámetros es lo caro —`is_quoted_printable` pasa a minúsculas y copia
        // los `prefijos`—, así que repetirlo en cada continuación es un tiempo
        // que crece como el **cuadrado del tamaño de la línea**: el mismo
        // defecto que este juntador vino a eliminar, abierto otra vez en la misma
        // función. Con 256 KiB de parámetros y cien mil continuaciones de la
        // 3.0 son 25 GB de copia por tarjeta.
        //
        // El guard también deja `scanned` y `quote_open` consistentes entre sí, que es
        // lo que hace que `quote_open` tenga que quedar pegado al `scanned`: `quote_open` es
        // el estado de las comillas **en `scanned`**, y cuando se encuentra el
        // `:` `scanned` no avanza, así que `quote_open = false` describía otra posición.
        // Sin búsqueda no hay dos estados que se puedan desincronizar.
        //
        // Lo que sí tiene que seguir buscando es la línea que **todavía no
        // encontró** su separador: `quoted_printable` sigue en `None` y `scanned` quedó
        // en el final del fragmento anterior, así que la continuación siguiente
        // mira desde ahí. Eso es lineal, y es el caso que prueba
        // `una_linea_sin_separador_sigue_buscando`.
        if quoted_printable.is_none() {
            if let Some(last) = lines.last() {
                // Sólo lo que todavía no se miró, y `quote_open` viaja de fragmento en
                // fragmento porque una comilla puede abrirse en uno y cerrarse en
                // el siguiente.
                let (at, still_open) = find_separator(last, scanned, quote_open);
                match at {
                    Some(at) => quoted_printable = Some(is_quoted_printable(&last[..at])),
                    None => scanned = last.len(),
                }
                quote_open = still_open;
            }
        }
    }

    lines
}

/// El primer `:` **fuera de comillas** desde `from`, y si queda alguna abierta.
///
/// Un parámetro puede traer un valor con dos puntos adentro —`X-TEST="a:b"`, que
/// es legal y lo exportan herramientas de verdad— y buscar a byte crudo se para
/// en el de adentro: los parámetros quedan `FN;X-TEST="a`, que no dicen
/// `quoted-printable`, la línea no se junta, y el valor queda con el `=` pegado
/// al final.
///
/// **Sólo las comillas dobles cuentan**, y es a propósito: es lo que hace
/// [`split_property`], y los dos tienen que cortar en el mismo punto o la línea junta y
/// después se parte distinto. En vCard el valor de un parámetro se entrecomilla
/// con `"`; un `'` es un carácter más del valor, y `X-TEST='a:b'` se parte en el
/// `:` de adentro, como siempre se partió.
///
/// **Se escanea desde `from` y no desde el principio.** Volver a recorrer lo
/// anterior en cada fragmento es un tiempo que crece como el cuadrado del
/// tamaño de la línea, que es justo lo que este juntador vino a arreglar.
///
/// El `bool` devuelto dice si quedó una comilla abierta: un parámetro partido a
/// la mitad puede abrirla en un fragmento y cerrarla en el siguiente.
pub(super) fn find_separator(text: &str, from: usize, open: bool) -> (Option<usize>, bool) {
    let mut inside = open;
    for (i, c) in text[from..].char_indices() {
        match c {
            '"' => inside = !inside,
            ':' if !inside => return (Some(from + i), false),
            _ => {}
        }
    }
    (None, inside)
}

/// Si los parámetros de una línea —lo que está antes de sus primeros dos
/// puntos— dicen que el valor va en `quoted-printable`.
///
/// Con eso no alcanza para juntar: además el valor tiene que terminar en `=`. El
/// base64 de una foto termina en `=` y no sigue, y un valor en
/// `quoted-printable` que termina donde termina tampoco.
fn is_quoted_printable(left: &str) -> bool {
    left.to_ascii_lowercase()
        .replace(' ', "")
        .contains("encoding=quoted-printable")
}

/// Una propiedad ya separada en sus partes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Property {
    /// El grupo que va antes del punto (`item1` en `item1.EMAIL`), en
    /// minúsculas. Vacío si no tiene.
    pub group: String,
    pub name: String,
    pub params: Vec<String>,
    pub value: String,
}

impl Property {
    /// El valor del parámetro `nombre=valor` que se pida, sin comillas y sin
    /// mirar mayúsculas en el nombre. El primero, si hay varios.
    pub fn param(&self, wanted: &str) -> Option<&str> {
        self.params.iter().find_map(|p| {
            let (name, value) = p.split_once('=')?;
            name.trim()
                .eq_ignore_ascii_case(wanted)
                .then(|| value.trim().trim_matches('"'))
        })
    }
}

/// Separa el grupo, el nombre y sus parámetros del valor.
///
/// Se corta por el **primer** dos puntos fuera de comillas: el valor puede
/// tener varios —`URL:https://x`— y cortar por el último dejaría la mitad de la
/// dirección en el nombre de la propiedad.
///
/// El nombre puede venir con un grupo adelante (`item1.EMAIL`), que ponen los
/// exportadores de Apple. El grupo va **aparte** y no pegado al nombre: pegado,
/// `item1.EMAIL` no se reconocería como un correo; tirado, se pierde la
/// etiqueta que Apple guarda en `item1.X-ABLabel`, que es la única que tienen
/// las webs, las fechas y los campos a medida de una tarjeta de iCloud.
pub fn split_property(line: &str) -> Option<Property> {
    let mut in_quotes = false;
    let cut = line.char_indices().find_map(|(i, c)| match c {
        '"' => {
            in_quotes = !in_quotes;
            None
        }
        ':' if !in_quotes => Some(i),
        _ => None,
    })?;

    let (left, right) = line.split_at(cut);
    let value = right[1..].to_string();

    let mut parts = left.split(';');
    let raw = parts.next()?.trim();
    // El grupo va antes de un punto. `X-ABLabel` de Apple viene así.
    let (group, name) = match raw.rsplit_once('.') {
        Some((group, name)) => (group.to_ascii_lowercase(), name),
        None => (String::new(), raw),
    };

    Some(Property {
        group,
        name: name.to_ascii_uppercase(),
        params: parts.map(|p| p.trim().to_string()).collect(),
        value,
    })
}

/// Deshace lo escapado de un valor de texto.
///
/// En vCard la coma, el punto y coma y el salto de línea van escapados. Sin
/// deshacerlo, una nota con una coma se muestra con la barra a la vista.
pub fn unescape_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();

    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') | Some('N') => out.push('\n'),
            Some(',') => out.push(','),
            Some(';') => out.push(';'),
            Some(':') => out.push(':'),
            Some('\\') => out.push('\\'),
            // Una barra que no escapa nada conocido se deja: es un dato de
            // alguien y tragárselo cambiaría el texto.
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }

    out
}

/// Los campos de un valor con varias partes, como `N` o `ADR`.
///
/// El separador es el punto y coma **sin escapar**: un apellido compuesto que
/// lleve uno escapado no puede partir el campo en dos.
pub fn split_fields(value: &str) -> Vec<String> {
    split_unescaped(value, ';')
        .into_iter()
        .map(|c| unescape_text(&c))
        .collect()
}

/// Los valores de una lista separada por comas, como `NICKNAME` o
/// `CATEGORIES`.
///
/// Igual que [`split_fields`] pero con la coma: la que va escapada (`\,`) es
/// parte del valor —«Pérez\, hijos» es una categoría, no dos— y la que no, lo
/// separa. Lo vacío se descarta.
pub fn split_list(value: &str) -> Vec<String> {
    split_unescaped(value, ',')
        .into_iter()
        .map(|v| unescape_text(v.trim()))
        .filter(|v| !v.trim().is_empty())
        .collect()
}

/// Parte por un separador **sin escapar**, dejando los escapes como vinieron:
/// desescapar antes de partir haría que el separador escapado partiera igual.
fn split_unescaped(value: &str, separator: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut escaped = false;

    for c in value.chars() {
        if escaped {
            current.push('\\');
            current.push(c);
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            c if c == separator => parts.push(std::mem::take(&mut current)),
            other => current.push(other),
        }
    }
    if escaped {
        current.push('\\');
    }
    parts.push(current);
    parts
}

/// La etiqueta de una propiedad: «casa», «trabajo», «celular».
///
/// Se leen las dos formas: `TYPE=HOME` de la 3.0 y la 4.0, y `HOME` suelto de
/// la 2.1. Sin la segunda, cualquier agenda exportada de un teléfono viejo
/// muestra todos los teléfonos sin etiqueta.
pub fn label_of(params: &[String]) -> String {
    let is_label = |p: &str| {
        let lower = p.to_ascii_lowercase();
        // Lo que no es una etiqueta para mostrar: cómo viene codificado, en qué
        // juego de caracteres, y cuál es el preferido.
        // Y en la 2.1, que los manda sueltos, la codificación sin su nombre.
        !matches!(
            lower.as_str(),
            "pref" | "internet" | "voice" | "x400" | "quoted-printable" | "base64" | "8bit"
        ) && !lower.starts_with("encoding=")
            && !lower.starts_with("charset=")
            && !lower.starts_with("value=")
            && !lower.starts_with("pref=")
    };

    params
        .iter()
        .flat_map(|p| match p.split_once('=') {
            // `TYPE=HOME,VOICE` puede traer varias juntas.
            Some((name, value)) if name.eq_ignore_ascii_case("type") => {
                value.split(',').map(str::to_string).collect::<Vec<_>>()
            }
            // Cualquier otro parámetro con nombre —`VALUE=`, `LABEL=`,
            // `X-SERVICE-TYPE=`, `PID=`— no es una etiqueta: dice cómo leer el
            // valor, y mostrarlo como «label="Av. Siempreviva 742"» antes de
            // una dirección sería ruido.
            Some(_) => Vec::new(),
            // Suelto, como en la 2.1: `TEL;HOME:`.
            None => vec![p.clone()],
        })
        .map(|p| p.trim().trim_matches('"').to_string())
        .find(|p| is_label(p))
        .map(|p| p.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Deshace `quoted-printable`, que es como la 2.1 manda los acentos.
pub fn decode_quoted_printable(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'=' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        let hex_digit = |b: Option<&u8>| (*b? as char).to_digit(16);
        match (hex_digit(bytes.get(i + 1)), hex_digit(bytes.get(i + 2))) {
            (Some(high), Some(lower)) => {
                out.push((high * 16 + lower) as u8);
                i += 3;
            }
            // Un `=` que no es un escape válido se deja como está.
            _ => {
                out.push(b'=');
                i += 1;
            }
        }
    }

    out
}

/// El juego de caracteres que declaró la propiedad, si declaró alguno.
fn declared_charset(property: &Property) -> Option<&str> {
    property.params.iter().find_map(|p| {
        let (name, value) = p.split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("charset")
            .then(|| value.trim().trim_matches('"'))
    })
}

/// Deshace la codificación de transporte y el juego de caracteres, **y nada
/// más**.
///
/// Separado del desescape a propósito. Un valor con varias partes —`N`, `ADR`,
/// `ORG`— se parte por los puntos y coma **después** de decodificar y **antes**
/// de desescapar: al revés, un `N;ENCODING=QUOTED-PRINTABLE:P=E9rez;Ana;;;`
/// mostraba «P=E9rez» y ordenaba la agenda por eso.
pub fn decoded(property: &Property) -> String {
    let has = |what: &str| {
        property
            .params
            .iter()
            .any(|p| p.to_ascii_lowercase().replace(' ', "") == what)
    };

    let declared = declared_charset(property);
    let quoted_printable = has("encoding=quoted-printable") || has("quoted-printable");

    // Sin `quoted-printable`, el valor ya es texto, y declare lo que declare: la
    // tarjeta llegó como `String` —el cuerpo de la respuesta pasó a UTF-8 antes
    // de leer el XML—, así que sus bytes son UTF-8 y no los del juego declarado.
    //
    // **Volver a decodificarlos con el juego declarado los rompía.** Con
    // `!quoted_printable && declared.is_some()` se hacía, y una tarjeta 3.0 o 4.0
    // —que declara `CHARSET=UTF-8` a veces, y siempre viene ya en texto—
    // pasaba dos veces por el mismo paso: `Łukasz` salía «Ĺ», un control
    // invisible y «ukasz». El juego declarado sólo describe los bytes que salen
    // de **deshacer el `quoted-printable`**, y a ésos se les aplica y a ésos
    // nada más.
    if !quoted_printable {
        return property.value.clone();
    }

    bytes_to_text(&decode_quoted_printable(&property.value), declared)
}

/// Pasa bytes a texto según el juego que declaró la tarjeta.
///
/// **Se respeta lo declarado.** Una 2.1 puede decir `CHARSET=WINDOWS-1252`, y
/// suponer latin-1 ahí convierte las comillas tipográficas y el guión largo en
/// caracteres de control invisibles.
///
/// Sin juego declarado, o con uno que no se conoce: se prueba UTF-8 y se cae a
/// Windows-1252, que es lo que manda una agenda exportada hace quince años. Sin
/// ese respaldo, cada acento sale como un rombo.
pub fn bytes_to_text(bytes: &[u8], charset: Option<&str>) -> String {
    let declared_encoding =
        charset.and_then(|j| encoding_rs::Encoding::for_label(j.trim().as_bytes()));

    let encoding = match declared_encoding {
        // `us-ascii` con bytes que no son ASCII no es us-ascii: el estándar de
        // codificaciones lo trata como Windows-1252, que decodifica cualquier
        // byte sin dar error, así que un valor en UTF-8 mal declarado saldría
        // con «Ã³» y nada lo notaría.
        Some(c) if c == encoding_rs::WINDOWS_1252 && std::str::from_utf8(bytes).is_ok() => {
            encoding_rs::UTF_8
        }
        Some(c) => c,
        None if std::str::from_utf8(bytes).is_ok() => encoding_rs::UTF_8,
        None => encoding_rs::WINDOWS_1252,
    };

    encoding.decode(bytes).0.into_owned()
}

/// El valor de una propiedad, listo para mostrar.
pub fn display_value(property: &Property) -> String {
    unescape_text(&decoded(property))
}

/// Separa las tarjetas de una respuesta que trae varias pegadas.
///
/// Un servidor puede devolver un archivo con muchas, y hay libretas exportadas
/// que son un solo archivo con miles. Sin separarlas, se leería una sola con
/// los datos de todas mezclados.
pub fn split_cards(raw: &str) -> Vec<String> {
    let mut cards = Vec::new();
    let mut current: Option<Vec<String>> = None;

    for line in unfold_lines(raw) {
        let upper = line.trim().to_ascii_uppercase();
        if upper == "BEGIN:VCARD" {
            current = Some(vec![line]);
            continue;
        }
        if upper == "END:VCARD" {
            if let Some(mut lines) = current.take() {
                lines.push(line);
                cards.push(lines.join("\r\n"));
            }
            continue;
        }
        if let Some(lines) = current.as_mut() {
            lines.push(line);
        }
    }

    // Una tarjeta sin su `END` está mal formada, pero lo que se leyó se
    // aprovecha: es preferible a perder un contacto por dos palabras que
    // faltaron.
    if let Some(lines) = current {
        cards.push(lines.join("\r\n"));
    }

    cards
}
