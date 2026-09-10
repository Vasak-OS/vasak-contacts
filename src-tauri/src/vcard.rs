//! Interpretar una tarjeta de contacto.
//!
//! ── De dónde viene esto ─────────────────────────────────────────────────────
//!
//! De la libreta de la persona, pero **lo escribió cualquiera**: una tarjeta
//! puede haber llegado adjunta a un correo, importada de un teléfono viejo, o
//! sincronizada desde un servidor compartido. No es contenido de confianza sólo
//! por estar en la libreta de alguien.
//!
//! Por eso el parseo tiene topes, no tiene `unsafe`, y todo lo que no se
//! entiende devuelve algo razonable en vez de cortar. Una tarjeta rota no puede
//! impedir ver las otras trescientas.
//!
//! ── Las tres versiones ──────────────────────────────────────────────────────
//!
//! Conviven la 2.1, la 3.0 y la 4.0, y las diferencias que importan son pocas
//! pero muerden:
//!
//! - En **2.1** los parámetros van sueltos (`TEL;HOME;VOICE:`) y el texto puede
//!   venir en `quoted-printable`. La escriben los teléfonos viejos y los
//!   exportadores de agendas de hace veinte años, que es justo lo que la gente
//!   tiene guardado.
//! - En **3.0** los parámetros llevan nombre (`TEL;TYPE=HOME:`) y el juego de
//!   caracteres puede ser cualquiera.
//! - En **4.0** todo es UTF-8 y las direcciones llevan `mailto:`.
//!
//! Se leen las tres. Escribir es otra cosa y todavía no se hace.

use serde::{Deserialize, Serialize};

/// Cuántas propiedades se leen de una tarjeta.
///
/// Un contacto real tiene decenas. Mil es un archivo armado para hacer trabajar
/// al programa, o una tarjeta con la foto partida en pedazos — que igual no se
/// muestra.
const MAX_PROPIEDADES: usize = 1000;

/// Tope de un valor que se muestra.
///
/// Un nombre de diez mil caracteres no es un nombre: es algo que va a romper la
/// lista al dibujarla.
const MAX_VALOR: usize = 4096;

/// Una dirección de correo, un teléfono, o cualquier cosa con una etiqueta.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dato {
    /// «casa», «trabajo», «celular»… tal como lo escribió quien hizo la
    /// tarjeta, en minúsculas. Vacío si no dijo nada.
    pub tipo: String,
    pub valor: String,
}

/// Un contacto, listo para mostrar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contacto {
    /// El identificador de la tarjeta dentro de la libreta.
    pub uid: String,
    /// Cómo se llama, para mostrar.
    pub nombre: String,
    /// Para ordenar: «Pérez, Ana» en vez de «Ana Pérez».
    ///
    /// Va aparte porque ordenar por el nombre que se muestra pone a todas las
    /// Anas juntas y a los Pérez desparramados, que no es como nadie busca a
    /// alguien en una agenda.
    pub orden: String,
    pub correos: Vec<Dato>,
    pub telefonos: Vec<Dato>,
    #[serde(default)]
    pub organizacion: String,
    #[serde(default)]
    pub notas: String,
    /// La dirección de la tarjeta en el servidor, para volver a buscarla.
    #[serde(default)]
    pub url: String,
}

// ---------------------------------------------------------------------------
// Las líneas
// ---------------------------------------------------------------------------

/// Junta las líneas partidas de una tarjeta.
///
/// El formato corta a 75 octetos y sigue en la siguiente con un espacio o una
/// tabulación adelante. Sin volver a juntarlas, un nombre largo aparece cortado
/// y una foto en base64 —que ocupa cientos de líneas— se interpreta como cientos
/// de propiedades basura.
pub fn unir_lineas(texto: &str) -> Vec<String> {
    let mut lineas: Vec<String> = Vec::new();

    for cruda in texto.split('\n') {
        let linea = cruda.strip_suffix('\r').unwrap_or(cruda);
        match linea.strip_prefix([' ', '\t']) {
            Some(continuacion) => {
                if let Some(ultima) = lineas.last_mut() {
                    ultima.push_str(continuacion);
                    continue;
                }
                lineas.push(continuacion.to_string());
            }
            None => lineas.push(linea.to_string()),
        }
    }

    lineas
}

/// Una propiedad ya separada en sus partes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Propiedad {
    pub nombre: String,
    pub parametros: Vec<String>,
    pub valor: String,
}

/// Separa el nombre y sus parámetros del valor.
///
/// Se corta por el **primer** dos puntos fuera de comillas: el valor puede
/// tener varios —`URL:https://x`— y cortar por el último dejaría la mitad de la
/// dirección en el nombre de la propiedad.
///
/// El nombre puede venir con un grupo adelante (`item1.EMAIL`), que ponen los
/// exportadores de Apple. El grupo se descarta: sirve para relacionar
/// propiedades entre sí y acá no se usa, pero dejarlo pegado haría que
/// `item1.EMAIL` no se reconociera como un correo.
pub fn partir(linea: &str) -> Option<Propiedad> {
    let mut entre_comillas = false;
    let corte = linea.char_indices().find_map(|(i, c)| match c {
        '"' => {
            entre_comillas = !entre_comillas;
            None
        }
        ':' if !entre_comillas => Some(i),
        _ => None,
    })?;

    let (izquierda, derecha) = linea.split_at(corte);
    let valor = derecha[1..].to_string();

    let mut partes = izquierda.split(';');
    let crudo = partes.next()?.trim();
    // El grupo va antes de un punto. `X-ABLabel` de Apple viene así.
    let nombre = crudo.rsplit('.').next().unwrap_or(crudo).to_ascii_uppercase();

    Some(Propiedad {
        nombre,
        parametros: partes.map(|p| p.trim().to_string()).collect(),
        valor,
    })
}

/// Deshace lo escapado de un valor de texto.
///
/// En vCard la coma, el punto y coma y el salto de línea van escapados. Sin
/// deshacerlo, una nota con una coma se muestra con la barra a la vista.
pub fn texto_de(valor: &str) -> String {
    let mut salida = String::with_capacity(valor.len());
    let mut chars = valor.chars();

    while let Some(c) = chars.next() {
        if c != '\\' {
            salida.push(c);
            continue;
        }
        match chars.next() {
            Some('n') | Some('N') => salida.push('\n'),
            Some(',') => salida.push(','),
            Some(';') => salida.push(';'),
            Some(':') => salida.push(':'),
            Some('\\') => salida.push('\\'),
            // Una barra que no escapa nada conocido se deja: es un dato de
            // alguien y tragárselo cambiaría el texto.
            Some(otro) => {
                salida.push('\\');
                salida.push(otro);
            }
            None => salida.push('\\'),
        }
    }

    salida
}

/// Los campos de un valor con varias partes, como `N` o `ADR`.
///
/// El separador es el punto y coma **sin escapar**: un apellido compuesto que
/// lleve uno escapado no puede partir el campo en dos.
pub fn campos_de(valor: &str) -> Vec<String> {
    let mut campos = Vec::new();
    let mut actual = String::new();
    let mut escapado = false;

    for c in valor.chars() {
        if escapado {
            actual.push('\\');
            actual.push(c);
            escapado = false;
            continue;
        }
        match c {
            '\\' => escapado = true,
            ';' => campos.push(std::mem::take(&mut actual)),
            otro => actual.push(otro),
        }
    }
    if escapado {
        actual.push('\\');
    }
    campos.push(actual);

    campos.into_iter().map(|c| texto_de(&c)).collect()
}

/// La etiqueta de una propiedad: «casa», «trabajo», «celular».
///
/// Se leen las dos formas: `TYPE=HOME` de la 3.0 y la 4.0, y `HOME` suelto de
/// la 2.1. Sin la segunda, cualquier agenda exportada de un teléfono viejo
/// muestra todos los teléfonos sin etiqueta.
pub fn etiqueta_de(parametros: &[String]) -> String {
    let interesantes = |p: &str| {
        let bajo = p.to_ascii_lowercase();
        // Lo que no es una etiqueta para mostrar: cómo viene codificado, en qué
        // juego de caracteres, y cuál es el preferido.
        !matches!(bajo.as_str(), "pref" | "internet" | "voice" | "x400")
            && !bajo.starts_with("encoding=")
            && !bajo.starts_with("charset=")
            && !bajo.starts_with("value=")
            && !bajo.starts_with("pref=")
    };

    parametros
        .iter()
        .flat_map(|p| match p.split_once('=') {
            // `TYPE=HOME,VOICE` puede traer varias juntas.
            Some((nombre, valor)) if nombre.eq_ignore_ascii_case("type") => {
                valor.split(',').map(str::to_string).collect::<Vec<_>>()
            }
            Some(_) => vec![p.clone()],
            None => vec![p.clone()],
        })
        .map(|p| p.trim().trim_matches('"').to_string())
        .find(|p| interesantes(p))
        .map(|p| p.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Deshace `quoted-printable`, que es como la 2.1 manda los acentos.
pub fn imprimible_de(texto: &str) -> Vec<u8> {
    let bytes = texto.as_bytes();
    let mut salida = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'=' {
            salida.push(bytes[i]);
            i += 1;
            continue;
        }
        let digito = |b: Option<&u8>| (*b? as char).to_digit(16);
        match (digito(bytes.get(i + 1)), digito(bytes.get(i + 2))) {
            (Some(alto), Some(bajo)) => {
                salida.push((alto * 16 + bajo) as u8);
                i += 3;
            }
            // Un `=` que no es un escape válido se deja como está.
            _ => {
                salida.push(b'=');
                i += 1;
            }
        }
    }

    salida
}

/// Pasa un valor a texto según lo que declare la propiedad.
///
/// Sólo hace falta para la 2.1, que es la que manda `quoted-printable` y juegos
/// de caracteres distintos de UTF-8. En la 3.0 y la 4.0 el valor ya es texto.
pub fn valor_legible(propiedad: &Propiedad) -> String {
    let tiene = |que: &str| {
        propiedad
            .parametros
            .iter()
            .any(|p| p.to_ascii_lowercase().replace(' ', "") == que)
    };

    if !tiene("encoding=quoted-printable") && !tiene("quoted-printable") {
        return texto_de(&propiedad.valor);
    }

    let bytes = imprimible_de(&propiedad.valor);
    // El juego declarado, y si no se conoce, UTF-8 con respaldo en latin-1: es
    // lo que manda una agenda exportada hace quince años, y sin el respaldo cada
    // acento sale como un rombo.
    let texto = match std::str::from_utf8(&bytes) {
        Ok(t) => t.to_string(),
        Err(_) => bytes.iter().map(|b| char::from(*b)).collect(),
    };
    texto_de(&texto)
}

// ---------------------------------------------------------------------------
// La tarjeta entera
// ---------------------------------------------------------------------------

/// Lee un contacto de una tarjeta.
///
/// `None` si no hay nada que mostrar: una tarjeta sin nombre y sin datos ocupa
/// lugar en la lista y no sirve para nada.
pub fn contacto_de(crudo: &str, url: &str) -> Option<Contacto> {
    let mut contacto = Contacto {
        url: url.to_string(),
        ..Default::default()
    };
    let mut nombre_estructurado: Vec<String> = Vec::new();

    for (i, linea) in unir_lineas(crudo).into_iter().enumerate() {
        if i >= MAX_PROPIEDADES {
            break;
        }
        let Some(propiedad) = partir(&linea) else {
            continue;
        };

        match propiedad.nombre.as_str() {
            "UID" => contacto.uid = recortado(&valor_legible(&propiedad)),
            "FN" => contacto.nombre = recortado(&valor_legible(&propiedad)),
            "N" => nombre_estructurado = campos_de(&propiedad.valor),
            "EMAIL" => {
                // En la 4.0 la dirección viene como `mailto:ana@x`. Dejarlo
                // haría que el botón de escribirle abriera «mailto:mailto:…».
                let valor = valor_legible(&propiedad);
                let valor = valor
                    .strip_prefix("mailto:")
                    .or_else(|| valor.strip_prefix("MAILTO:"))
                    .unwrap_or(&valor)
                    .trim()
                    .to_string();
                agregar(&mut contacto.correos, &propiedad, valor);
            }
            "TEL" => {
                let valor = valor_legible(&propiedad);
                let valor = valor.strip_prefix("tel:").unwrap_or(&valor).trim().to_string();
                agregar(&mut contacto.telefonos, &propiedad, valor);
            }
            "ORG" => {
                // `ORG` trae la empresa y sus divisiones separadas por punto y
                // coma. Se muestran juntas y no sólo la primera: «Vasak Group»
                // y «Vasak Group, Soporte» son cosas distintas.
                contacto.organizacion = recortado(
                    &campos_de(&propiedad.valor)
                        .into_iter()
                        .filter(|c| !c.trim().is_empty())
                        .collect::<Vec<_>>()
                        .join(", "),
                );
            }
            "NOTE" => contacto.notas = recortado(&valor_legible(&propiedad)),
            _ => {}
        }
    }

    // El nombre que se muestra: el `FN` si está, y si no se arma con el `N`.
    // Una tarjeta sin `FN` es inválida según el estándar y aparece igual, así
    // que armarlo es la diferencia entre ver a alguien y ver un renglón vacío.
    if contacto.nombre.trim().is_empty() {
        contacto.nombre = nombre_para_mostrar(&nombre_estructurado);
    }
    contacto.orden = nombre_para_ordenar(&nombre_estructurado, &contacto.nombre);

    let hay_algo = !contacto.nombre.trim().is_empty()
        || !contacto.correos.is_empty()
        || !contacto.telefonos.is_empty();
    hay_algo.then_some(contacto)
}

fn agregar(destino: &mut Vec<Dato>, propiedad: &Propiedad, valor: String) {
    if valor.is_empty() {
        return;
    }
    destino.push(Dato {
        tipo: etiqueta_de(&propiedad.parametros),
        valor: recortado(&valor),
    });
}

/// Un valor que no rompa la lista al dibujarla.
fn recortado(valor: &str) -> String {
    if valor.len() <= MAX_VALOR {
        return valor.to_string();
    }
    let mut corte = MAX_VALOR;
    while corte > 0 && !valor.is_char_boundary(corte) {
        corte -= 1;
    }
    format!("{}…", &valor[..corte])
}

/// Arma «Ana Pérez» a partir del `N`, que viene al revés y por partes.
///
/// El orden del campo es apellido, nombre, segundos nombres, tratamiento y
/// sufijo. Mostrarlo tal cual daría «Pérez;Ana;;Sra.;».
fn nombre_para_mostrar(campos: &[String]) -> String {
    let campo = |i: usize| campos.get(i).map(String::as_str).unwrap_or("").trim();
    [campo(3), campo(1), campo(2), campo(0), campo(4)]
        .iter()
        .filter(|p| !p.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
}

/// «Pérez, Ana»: cómo se busca a alguien en una agenda.
///
/// Ordenar por el nombre que se muestra pone a todas las Anas juntas y a los
/// Pérez desparramados, que no es como nadie busca.
fn nombre_para_ordenar(campos: &[String], para_mostrar: &str) -> String {
    let campo = |i: usize| campos.get(i).map(String::as_str).unwrap_or("").trim();
    let apellido = campo(0);
    let nombre = campo(1);

    match (apellido.is_empty(), nombre.is_empty()) {
        (false, false) => format!("{apellido}, {nombre}"),
        (false, true) => apellido.to_string(),
        // Sin apellido, se ordena por lo que se muestra: es lo único que hay.
        _ => para_mostrar.to_string(),
    }
}

/// Separa las tarjetas de una respuesta que trae varias pegadas.
///
/// Un servidor puede devolver un archivo con muchas, y hay libretas exportadas
/// que son un solo archivo con miles. Sin separarlas, se leería una sola con
/// los datos de todas mezclados.
pub fn tarjetas_de(crudo: &str) -> Vec<String> {
    let mut tarjetas = Vec::new();
    let mut actual: Option<Vec<String>> = None;

    for linea in unir_lineas(crudo) {
        let mayusculas = linea.trim().to_ascii_uppercase();
        if mayusculas == "BEGIN:VCARD" {
            actual = Some(vec![linea]);
            continue;
        }
        if mayusculas == "END:VCARD" {
            if let Some(mut lineas) = actual.take() {
                lineas.push(linea);
                tarjetas.push(lineas.join("\r\n"));
            }
            continue;
        }
        if let Some(lineas) = actual.as_mut() {
            lineas.push(linea);
        }
    }

    // Una tarjeta sin su `END` está mal formada, pero lo que se leyó se
    // aprovecha: es preferible a perder un contacto por dos palabras que
    // faltaron.
    if let Some(lineas) = actual {
        tarjetas.push(lineas.join("\r\n"));
    }

    tarjetas
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANA: &str = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:abc-123\r\n\
        FN:Ana Pérez\r\nN:Pérez;Ana;;;\r\n\
        EMAIL;TYPE=WORK:ana@ejemplo.com\r\n\
        TEL;TYPE=CELL:+54 11 5555-5555\r\n\
        ORG:Vasak Group;Soporte\r\nEND:VCARD\r\n";

    #[test]
    fn se_lee_una_tarjeta() {
        let c = contacto_de(ANA, "https://x/ana.vcf").unwrap();

        assert_eq!(c.uid, "abc-123");
        assert_eq!(c.nombre, "Ana Pérez");
        assert_eq!(c.correos[0].valor, "ana@ejemplo.com");
        assert_eq!(c.correos[0].tipo, "work");
        assert_eq!(c.telefonos[0].valor, "+54 11 5555-5555");
        assert_eq!(c.url, "https://x/ana.vcf");
    }

    // ── Las líneas ─────────────────────────────────────────────────────────

    /// Sin volver a juntarlas, un nombre largo aparece cortado y una foto en
    /// base64 —que ocupa cientos de líneas— se interpreta como cientos de
    /// propiedades basura.
    #[test]
    fn las_lineas_partidas_se_vuelven_a_juntar() {
        let crudo = "FN:Ana\r\n  Pérez\r\nUID:1\r\n";
        let lineas = unir_lineas(crudo);
        assert_eq!(lineas[0], "FN:Ana Pérez");
        assert_eq!(lineas[1], "UID:1");
    }

    /// El carácter que pliega **se va**: en el test de arriba el espacio que
    /// sobrevive es el segundo, el que el nombre tenía de verdad.
    #[test]
    fn el_caracter_que_pliega_no_deja_espacio() {
        assert_eq!(unir_lineas("FN:Ana\r\n\tPérez")[0], "FN:AnaPérez");
    }

    /// El valor puede tener dos puntos —una URL— así que el corte va por el
    /// primero. Cortar por el último dejaría media dirección en el nombre.
    #[test]
    fn la_linea_se_corta_por_el_primer_dos_puntos() {
        let p = partir("URL:https://ejemplo.com/ana").unwrap();
        assert_eq!(p.nombre, "URL");
        assert_eq!(p.valor, "https://ejemplo.com/ana");
    }

    /// Los exportadores de Apple ponen un grupo adelante. Dejarlo pegado haría
    /// que `item1.EMAIL` no se reconociera como un correo.
    #[test]
    fn el_grupo_de_apple_no_esconde_la_propiedad() {
        let p = partir("item1.EMAIL;TYPE=HOME:ana@x.com").unwrap();
        assert_eq!(p.nombre, "EMAIL");

        let c = contacto_de(
            "BEGIN:VCARD\r\nFN:Ana\r\nitem1.EMAIL;TYPE=HOME:ana@x.com\r\nEND:VCARD",
            "",
        )
        .unwrap();
        assert_eq!(c.correos.len(), 1);
    }

    #[test]
    fn el_texto_se_desescapa() {
        assert_eq!(texto_de(r"Pérez\, Ana"), "Pérez, Ana");
        assert_eq!(texto_de(r"uno\ndos"), "uno\ndos");
        assert_eq!(texto_de(r"punto\; y coma"), "punto; y coma");
    }

    /// Un apellido compuesto con un punto y coma escapado no puede partir el
    /// campo en dos.
    #[test]
    fn un_punto_y_coma_escapado_no_parte_el_campo() {
        assert_eq!(campos_de(r"Pérez\;Gómez;Ana"), vec!["Pérez;Gómez", "Ana"]);
        assert_eq!(campos_de("Pérez;Ana;;;"), vec!["Pérez", "Ana", "", "", ""]);
    }

    // ── Las tres versiones ─────────────────────────────────────────────────

    /// **Los teléfonos viejos escriben 2.1**, con los parámetros sueltos. Sin
    /// leerlos, cualquier agenda exportada de uno muestra todos los teléfonos
    /// sin etiqueta.
    #[test]
    fn se_lee_la_etiqueta_de_las_tres_versiones() {
        assert_eq!(etiqueta_de(&["TYPE=WORK".into()]), "work");
        assert_eq!(etiqueta_de(&["HOME".into()]), "home");
        assert_eq!(etiqueta_de(&["TYPE=\"HOME\"".into()]), "home");
        // Y con varias juntas, la primera que sirva.
        assert_eq!(etiqueta_de(&["TYPE=VOICE,HOME".into()]), "home");
    }

    /// Lo que no es una etiqueta para mostrar no puede terminar en pantalla:
    /// «internet» no dice nada de un correo, y «pref» tampoco.
    #[test]
    fn lo_que_no_es_una_etiqueta_no_se_muestra_como_tal() {
        assert_eq!(etiqueta_de(&["INTERNET".into()]), "");
        assert_eq!(etiqueta_de(&["PREF".into()]), "");
        assert_eq!(etiqueta_de(&["ENCODING=QUOTED-PRINTABLE".into()]), "");
        assert_eq!(etiqueta_de(&["CHARSET=UTF-8".into()]), "");
        assert_eq!(etiqueta_de(&[]), "");
        // Pero si además hay una de verdad, ésa sí.
        assert_eq!(etiqueta_de(&["INTERNET".into(), "HOME".into()]), "home");
    }

    /// La 2.1 manda los acentos en `quoted-printable`. Sin deshacerlo, media
    /// agenda en español se ve con signos de igual en el medio de los nombres.
    #[test]
    fn una_tarjeta_vieja_con_acentos_se_lee() {
        let vieja = "BEGIN:VCARD\r\nVERSION:2.1\r\n\
            FN;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:Ana P=C3=A9rez\r\n\
            TEL;HOME:11-5555\r\nEND:VCARD";

        let c = contacto_de(vieja, "").unwrap();
        assert_eq!(c.nombre, "Ana Pérez");
        assert_eq!(c.telefonos[0].tipo, "home");
    }

    /// Y si los bytes no son UTF-8, se leen como latin-1 en vez de mostrar
    /// rombos: es lo que manda una agenda exportada hace quince años.
    #[test]
    fn una_tarjeta_vieja_en_latin1_tambien() {
        let vieja = "BEGIN:VCARD\r\nVERSION:2.1\r\n\
            FN;ENCODING=QUOTED-PRINTABLE:Ana P=E9rez\r\nEND:VCARD";
        assert_eq!(contacto_de(vieja, "").unwrap().nombre, "Ana Pérez");
    }

    /// En la 4.0 la dirección viene con `mailto:` adelante. Dejarlo haría que
    /// el botón de escribirle abriera «mailto:mailto:…».
    #[test]
    fn el_mailto_de_la_version_4_no_queda_pegado() {
        let nueva = "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Ana\r\n\
            EMAIL:mailto:ana@ejemplo.com\r\nTEL:tel:+541155555555\r\nEND:VCARD";

        let c = contacto_de(nueva, "").unwrap();
        assert_eq!(c.correos[0].valor, "ana@ejemplo.com");
        assert_eq!(c.telefonos[0].valor, "+541155555555");
    }

    // ── El nombre ──────────────────────────────────────────────────────────

    /// Una tarjeta sin `FN` es inválida según el estándar y aparece igual.
    /// Armar el nombre con el `N` es la diferencia entre ver a alguien y ver un
    /// renglón vacío.
    #[test]
    fn sin_fn_el_nombre_se_arma_con_el_n() {
        let sin_fn = "BEGIN:VCARD\r\nN:Pérez;Ana;María;Sra.;\r\nEND:VCARD";
        assert_eq!(contacto_de(sin_fn, "").unwrap().nombre, "Sra. Ana María Pérez");
    }

    /// Ordenar por el nombre que se muestra pone a todas las Anas juntas y a
    /// los Pérez desparramados, que no es como nadie busca en una agenda.
    #[test]
    fn se_ordena_por_apellido() {
        assert_eq!(contacto_de(ANA, "").unwrap().orden, "Pérez, Ana");

        // Sin apellido se ordena por lo que se muestra: es lo único que hay.
        let solo_fn = "BEGIN:VCARD\r\nFN:Panadería del barrio\r\nEND:VCARD";
        assert_eq!(
            contacto_de(solo_fn, "").unwrap().orden,
            "Panadería del barrio"
        );
    }

    /// `ORG` trae la empresa y sus divisiones. «Vasak Group» y «Vasak Group,
    /// Soporte» son cosas distintas.
    #[test]
    fn la_organizacion_incluye_la_division() {
        assert_eq!(contacto_de(ANA, "").unwrap().organizacion, "Vasak Group, Soporte");
    }

    // ── Lo que llega roto ──────────────────────────────────────────────────

    /// Una tarjeta sin nada que mostrar ocupa lugar en la lista y no sirve para
    /// nada.
    #[test]
    fn una_tarjeta_vacia_no_es_un_contacto() {
        assert!(contacto_de("BEGIN:VCARD\r\nVERSION:3.0\r\nEND:VCARD", "").is_none());
        assert!(contacto_de("", "").is_none());
        assert!(contacto_de("no es una tarjeta", "").is_none());
    }

    /// Una tarjeta puede haber llegado adjunta a un correo o importada de un
    /// teléfono: no es contenido de confianza por estar en la libreta.
    #[test]
    fn lo_que_esta_roto_no_hace_caer_nada() {
        for basura in [":::", "FN:", ";;;", "BEGIN:VCARD", "\r\n\r\n", "\\", "N:;;;;;;;;;;"] {
            let _ = contacto_de(basura, "");
            let _ = tarjetas_de(basura);
        }
    }

    /// Un archivo armado para hacer trabajar al programa, o una tarjeta con la
    /// foto partida en pedazos: en los dos casos tiene que volver.
    #[test]
    fn una_tarjeta_desmedida_no_cuelga() {
        let enorme = "BEGIN:VCARD\r\nFN:Ana\r\n".to_string()
            + &"X-BASURA:algo\r\n".repeat(50_000)
            + "END:VCARD";
        let c = contacto_de(&enorme, "").unwrap();
        assert_eq!(c.nombre, "Ana");
    }

    /// Un nombre de diez mil caracteres no es un nombre: es algo que va a
    /// romper la lista al dibujarla.
    #[test]
    fn un_valor_desmedido_se_recorta_sin_partir_un_caracter() {
        let largo = format!("BEGIN:VCARD\r\nFN:{}\r\nEND:VCARD", "ñ".repeat(MAX_VALOR));
        let c = contacto_de(&largo, "").unwrap();

        assert!(c.nombre.len() <= MAX_VALOR + 4);
        assert!(c.nombre.ends_with('…'));
        assert!(!c.nombre.contains('\u{FFFD}'));
    }

    // ── Varias tarjetas ────────────────────────────────────────────────────

    /// Hay libretas exportadas que son un solo archivo con miles. Sin
    /// separarlas se leería una sola con los datos de todas mezclados.
    #[test]
    fn se_separan_las_tarjetas_de_un_archivo() {
        let dos = format!("{ANA}BEGIN:VCARD\r\nFN:Juan\r\nEND:VCARD\r\n");
        let tarjetas = tarjetas_de(&dos);

        assert_eq!(tarjetas.len(), 2);
        assert_eq!(contacto_de(&tarjetas[0], "").unwrap().nombre, "Ana Pérez");
        assert_eq!(contacto_de(&tarjetas[1], "").unwrap().nombre, "Juan");
    }

    /// Una tarjeta sin su `END` está mal formada, pero perder un contacto por
    /// dos palabras que faltaron sería peor.
    #[test]
    fn una_tarjeta_sin_cierre_se_aprovecha_igual() {
        let sin_cierre = "BEGIN:VCARD\r\nFN:Ana\r\n";
        let tarjetas = tarjetas_de(sin_cierre);
        assert_eq!(tarjetas.len(), 1);
        assert_eq!(contacto_de(&tarjetas[0], "").unwrap().nombre, "Ana");
    }
}
