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
}
