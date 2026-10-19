//! El código QR de la tarjeta de presentación.
//!
//! Todo local: la vCard reducida (`vcard::export`) se codifica con la caja
//! `qrcode`, que es Rust puro, y vuelve como SVG para que la ventana lo dibuje.
//! No hay red ni terceros: lo que se comparte es lo que la persona marcó, y sale
//! de la máquina sólo cuando otra persona lo escanea de la pantalla.
//!
//! Los errores vuelven como marcas que la ventana traduce, no como frases: el
//! idioma de la ventana es el de la sesión, el de este programa es el de su
//! compilación (mismo criterio que `commands::STORE_PERMISSION_DENIED`).

use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};

use crate::vcard::{reduced_vcard, QrCard};

/// No se marcó nada que compartir.
pub(crate) const QR_EMPTY: &str = "qr-empty";
/// Lo marcado no entra en un código que una cámara pueda leer.
pub(crate) const QR_TOO_LONG: &str = "qr-too-long";

/// Cuántos bytes de vCard se aceptan.
///
/// Con corrección `M` son unos 85×85 módulos —la versión 17—, que una cámara
/// lee desde la pantalla sin acercarse. El tope del estándar es cuatro veces
/// más, pero un código así es una nube de puntos: «poco lugar» no es que no
/// entre, es que no se pueda escanear (vasak-contacts#10).
const MAX_PAYLOAD_BYTES: usize = 580;

/// El SVG del código de un texto.
///
/// Sin colores propios: los módulos van en `currentColor` y el fondo es
/// transparente, así que los pone la ventana desde el esquema. La zona de
/// silencio (el borde claro que pide el estándar) va incluida en el `viewBox`.
pub fn qr_svg(payload: &str) -> Result<String, String> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(QR_TOO_LONG.into());
    }
    let code = QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::M)
        .map_err(|_| QR_TOO_LONG.to_string())?;
    Ok(code
        .render::<svg::Color>()
        .quiet_zone(true)
        .dark_color(svg::Color("currentColor"))
        .light_color(svg::Color("none"))
        .build())
}

/// El código de lo que se marcó, listo para dibujar.
pub fn card_svg(card: &QrCard) -> Result<String, String> {
    if card.is_empty() {
        return Err(QR_EMPTY.into());
    }
    qr_svg(&reduced_vcard(card))
}

/// Genera el QR de una vCard reducida. La ventana arma el [`QrCard`] con las
/// casillas marcadas; acá no se agrega nada a lo que trae.
#[tauri::command]
pub fn generate_vcard_qr(card: QrCard) -> Result<String, String> {
    card_svg(&card)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ana() -> QrCard {
        QrCard {
            name: "Ana Pérez".into(),
            phones: vec!["+54 11 5555-5555".into()],
            email: Some("ana@ejemplo.com".into()),
            organization: "Vasak Group".into(),
        }
    }

    #[test]
    fn una_tarjeta_normal_da_un_svg() {
        let svg = card_svg(&ana()).unwrap();
        assert!(svg.starts_with("<?xml") || svg.contains("<svg"));
        assert!(svg.contains("viewBox"));
    }

    #[test]
    fn el_svg_no_lleva_colores_propios() {
        let svg = card_svg(&ana()).unwrap();
        assert!(svg.contains("currentColor"));
        assert!(!svg.contains('#'), "un color literal: {svg}");
    }

    #[test]
    fn sin_nada_marcado_no_hay_codigo() {
        assert_eq!(card_svg(&QrCard::default()), Err(QR_EMPTY.to_string()));
    }

    #[test]
    fn una_tarjeta_demasiado_larga_se_dice() {
        let card = QrCard {
            name: "A".repeat(700),
            ..Default::default()
        };
        assert_eq!(card_svg(&card), Err(QR_TOO_LONG.to_string()));
    }
}
