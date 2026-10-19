//! La vCard reducida de la tarjeta de presentación.
//!
//! Un código QR tiene poco lugar: una tarjeta entera, con foto, direcciones y
//! notas, no entra, y una que entra pero queda densa no la lee ninguna cámara.
//! Lo que va es lo que alguien necesita para anotarte: el nombre, uno o dos
//! teléfonos, un correo y la organización. **Nunca la foto.**
//!
//! Esto no es el serializador completo de `vasak-accounts` (el que escribe las
//! tarjetas del almacén): ese vive en el servicio y esta aplicación no lo
//! enlaza, porque le pide al servicio que escriba y no escribe ella. Acá se arma
//! a mano la porción mínima de vCard 3.0 que hace falta, que es la que entienden
//! todas las agendas.
//!
//! Qué campos van lo decide la persona, con casillas, **antes** de generar el
//! código: la ventana arma un [`QrCard`] con lo marcado y acá no se agrega nada
//! que no venga en él.

use serde::Deserialize;

/// Cuántos teléfonos entran. Un tercero ya es una tarjeta de visita con el
/// reverso escrito.
pub const MAX_PHONES: usize = 2;

/// Lo que la persona marcó para compartir.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct QrCard {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub phones: Vec<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub organization: String,
}

impl QrCard {
    /// Un campo de texto de la tarjeta, sin blancos de más; `None` si queda vacío.
    fn clean(value: &str) -> Option<String> {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    }

    /// Si no hay nada que compartir: una tarjeta así no es un QR sino un cuadrado.
    pub fn is_empty(&self) -> bool {
        Self::clean(&self.name).is_none()
            && Self::clean(&self.organization).is_none()
            && self.email.as_deref().and_then(Self::clean).is_none()
            && self.phones.iter().all(|p| Self::clean(p).is_none())
    }
}

/// Escapa un valor de texto de vCard.
///
/// La barra invertida **primero**: si fuera después, la que agrega el escape de
/// `;` o de `,` se volvería a duplicar. Los saltos de línea pasan a `\n`, y el
/// resto de los caracteres de control se pierden: un `\r` suelto en un nombre es
/// la forma de colar una propiedad nueva en la tarjeta de otro.
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    // `\r\n` es un solo salto, no dos.
    for c in value.replace("\r\n", "\n").chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' | '\r' => out.push_str("\\n"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// La vCard 3.0 de lo que se marcó.
///
/// `N` va vacío (`N:;;;;`): el estándar lo pide, pero partir un nombre en
/// apellido y nombre es adivinar —«María de los Ángeles Pérez García»— y una
/// tarjeta con el apellido mal puesto es peor que una sin él. Las agendas
/// muestran `FN`.
///
/// shortcut: las líneas no se pliegan a 75 octetos (RFC 6350 §3.2); los lectores
/// de QR de teléfonos no lo exigen. Si algún lector las rechaza, se pliegan acá.
pub fn reduced_vcard(card: &QrCard) -> String {
    let mut lines = vec!["BEGIN:VCARD".to_string(), "VERSION:3.0".to_string()];
    let name = QrCard::clean(&card.name);
    // `FN` es obligatorio: sin nombre, el de la organización, y si tampoco hay,
    // se omite y la tarjeta que llegue lo dirá ella.
    let org = QrCard::clean(&card.organization);
    if let Some(shown) = name.as_ref().or(org.as_ref()) {
        lines.push(format!("FN:{}", escape(shown)));
        lines.push("N:;;;;".to_string());
    }
    if let Some(org) = &org {
        lines.push(format!("ORG:{}", escape(org)));
    }
    for phone in card
        .phones
        .iter()
        .filter_map(|p| QrCard::clean(p))
        .take(MAX_PHONES)
    {
        lines.push(format!("TEL:{}", escape(&phone)));
    }
    if let Some(email) = card.email.as_deref().and_then(QrCard::clean) {
        lines.push(format!("EMAIL:{}", escape(&email)));
    }
    lines.push("END:VCARD".to_string());
    lines.join("\r\n") + "\r\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ana() -> QrCard {
        QrCard {
            name: "Ana Pérez".into(),
            phones: vec!["+54 11 5555-5555".into(), "011 4444-4444".into()],
            email: Some("ana@ejemplo.com".into()),
            organization: "Vasak Group".into(),
        }
    }

    #[test]
    fn la_tarjeta_lleva_lo_marcado_y_nada_mas() {
        assert_eq!(
            reduced_vcard(&ana()),
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Ana Pérez\r\nN:;;;;\r\nORG:Vasak Group\r\n\
             TEL:+54 11 5555-5555\r\nTEL:011 4444-4444\r\nEMAIL:ana@ejemplo.com\r\nEND:VCARD\r\n"
        );
    }

    #[test]
    fn lo_que_no_se_marco_no_sale() {
        let out = reduced_vcard(&QrCard {
            name: "Ana Pérez".into(),
            ..Default::default()
        });
        assert!(!out.contains("TEL") && !out.contains("EMAIL") && !out.contains("ORG"));
    }

    #[test]
    fn nunca_lleva_foto() {
        assert!(!reduced_vcard(&ana()).contains("PHOTO"));
    }

    #[test]
    fn solo_entran_dos_telefonos() {
        let mut card = ana();
        card.phones.push("3333".into());
        assert_eq!(reduced_vcard(&card).matches("TEL:").count(), 2);
    }

    #[test]
    fn un_valor_con_saltos_o_separadores_no_cuela_propiedades() {
        let out = reduced_vcard(&QrCard {
            name: "Ana\r\nTEL:999".into(),
            organization: "A;B, C\\D".into(),
            ..Default::default()
        });
        assert!(!out.contains("\r\nTEL:999"));
        assert!(out.contains("FN:Ana\\nTEL:999\r\n"));
        assert!(out.contains("ORG:A\\;B\\, C\\\\D\r\n"));
    }

    #[test]
    fn sin_nombre_el_fn_es_la_organizacion() {
        let out = reduced_vcard(&QrCard {
            organization: "Vasak Group".into(),
            ..Default::default()
        });
        assert!(out.contains("FN:Vasak Group\r\n"));
    }

    #[test]
    fn una_tarjeta_de_blancos_esta_vacia() {
        assert!(QrCard {
            name: "  ".into(),
            phones: vec![" ".into()],
            email: Some("".into()),
            organization: "".into(),
        }
        .is_empty());
        assert!(!ana().is_empty());
    }

    /// Los nombres de propiedad de una vCard, en orden: lo que un lector de agendas
    /// vería como líneas distintas.
    fn propiedades(vcard: &str) -> Vec<String> {
        vcard
            .split("\r\n")
            .filter(|l| !l.is_empty())
            .map(|l| l.split(':').next().unwrap_or("").to_string())
            .collect()
    }

    /// Lo que cada campo puede traer de hostil: un salto de línea de cada clase y
    /// después una propiedad que la persona nunca marcó.
    const HOSTILES: [&str; 5] = [
        "x\r\nTEL:999",
        "x\nTEL:999",
        "x\rTEL:999",
        "x\u{85}TEL:999",
        "x\r\nEND:VCARD\r\nBEGIN:VCARD\r\nFN:otro",
    ];

    #[test]
    fn ningun_campo_cuela_una_propiedad() {
        for hostil in HOSTILES {
            let casos = [
                (
                    "nombre",
                    QrCard {
                        name: hostil.into(),
                        ..Default::default()
                    },
                ),
                (
                    "organización",
                    QrCard {
                        name: "Ana".into(),
                        organization: hostil.into(),
                        ..Default::default()
                    },
                ),
                (
                    "teléfono",
                    QrCard {
                        name: "Ana".into(),
                        phones: vec![hostil.into()],
                        ..Default::default()
                    },
                ),
                (
                    "correo",
                    QrCard {
                        name: "Ana".into(),
                        email: Some(hostil.into()),
                        ..Default::default()
                    },
                ),
            ];
            for (campo, card) in casos {
                let props = propiedades(&reduced_vcard(&card));
                // Sólo el marco, el nombre y a lo sumo la propiedad del campo probado.
                assert!(
                    props.iter().all(|p| [
                        "BEGIN", "VERSION", "FN", "N", "ORG", "TEL", "EMAIL", "END"
                    ]
                    .contains(&p.as_str())),
                    "{campo} con {hostil:?} coló {props:?}"
                );
                assert!(props.len() <= 7, "{campo} con {hostil:?} coló {props:?}");
                assert_eq!(
                    props.iter().filter(|p| *p == "TEL").count(),
                    card.phones.len(),
                    "{campo} con {hostil:?}"
                );
                assert_eq!(
                    props.iter().filter(|p| *p == "END").count(),
                    1,
                    "{campo} con {hostil:?}"
                );
                assert_eq!(
                    props.iter().filter(|p| *p == "BEGIN").count(),
                    1,
                    "{campo} con {hostil:?}"
                );
            }
        }
    }

    #[test]
    fn los_saltos_de_linea_son_siempre_los_del_marco() {
        let out = reduced_vcard(&QrCard {
            name: "a\r\nb\nc\rd".into(),
            ..Default::default()
        });
        // Quitados los `\r\n` del marco no queda ningún salto suelto.
        assert!(!out.replace("\r\n", "").contains(['\r', '\n']), "{out:?}");
    }

    #[test]
    fn los_caracteres_de_control_se_pierden() {
        let out = reduced_vcard(&QrCard {
            name: "A\u{0}\u{7}\u{1b}\u{7f}\u{85}B".into(),
            ..Default::default()
        });
        assert!(out.contains("FN:AB\r\n"), "{out:?}");
    }

    #[test]
    fn la_barra_invertida_se_escapa_antes_que_los_separadores() {
        // `a\;b` son tres caracteres de la persona: la barra, el `;` y la `b`.
        // Escapar la barra después duplicaría la que agrega el escape del `;`.
        let out = reduced_vcard(&QrCard {
            name: "a\\;b".into(),
            ..Default::default()
        });
        assert!(out.contains("FN:a\\\\\\;b\r\n"), "{out:?}");
    }

    #[test]
    fn el_telefono_y_el_correo_tambien_se_escapan() {
        let out = reduced_vcard(&QrCard {
            name: "Ana".into(),
            phones: vec!["1;2,3".into()],
            email: Some("a,b@x;y".into()),
            ..Default::default()
        });
        assert!(out.contains("TEL:1\\;2\\,3\r\n"), "{out:?}");
        assert!(out.contains("EMAIL:a\\,b@x\\;y\r\n"), "{out:?}");
    }

    #[test]
    fn los_telefonos_vacios_no_ocupan_un_lugar_de_los_dos() {
        let out = reduced_vcard(&QrCard {
            name: "Ana".into(),
            phones: vec![
                "".into(),
                "  ".into(),
                "111".into(),
                "222".into(),
                "333".into(),
            ],
            ..Default::default()
        });
        assert!(
            out.contains("TEL:111\r\n") && out.contains("TEL:222\r\n"),
            "{out:?}"
        );
        assert!(!out.contains("TEL:333"), "{out:?}");
    }

    #[test]
    fn un_correo_de_blancos_no_sale() {
        let out = reduced_vcard(&QrCard {
            name: "Ana".into(),
            email: Some("   ".into()),
            ..Default::default()
        });
        assert!(!out.contains("EMAIL"), "{out:?}");
    }

    #[test]
    fn los_blancos_de_los_costados_no_viajan() {
        let out = reduced_vcard(&QrCard {
            name: "  Ana  ".into(),
            email: Some(" a@x.com ".into()),
            ..Default::default()
        });
        assert!(
            out.contains("FN:Ana\r\n") && out.contains("EMAIL:a@x.com\r\n"),
            "{out:?}"
        );
    }

    #[test]
    fn el_texto_que_no_es_ascii_llega_entero() {
        let out = reduced_vcard(&QrCard {
            name: "María Ñandú 日本 😀".into(),
            ..Default::default()
        });
        assert!(out.contains("FN:María Ñandú 日本 😀\r\n"), "{out:?}");
    }

    #[test]
    fn con_un_solo_correo_la_tarjeta_no_esta_vacia() {
        assert!(!QrCard {
            email: Some("a@x.com".into()),
            ..Default::default()
        }
        .is_empty());
        assert!(!QrCard {
            phones: vec!["".into(), "111".into()],
            ..Default::default()
        }
        .is_empty());
    }
}
