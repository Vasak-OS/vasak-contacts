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

    /// Cuánto mide cada módulo en el `viewBox` que dibuja la caja `qrcode`.
    const PX_POR_MODULO: usize = 8;

    /// El lado, en módulos, que dice el `viewBox` del SVG (zona de silencio incluida).
    fn lado_del_svg(svg: &str) -> usize {
        let resto = svg.split("viewBox=\"0 0 ").nth(1).expect("sin viewBox");
        let px: usize = resto.split(' ').next().unwrap().parse().unwrap();
        assert_eq!(
            px % PX_POR_MODULO,
            0,
            "el viewBox no es de módulos enteros: {px}"
        );
        px / PX_POR_MODULO
    }

    #[test]
    fn el_tope_son_580_bytes_justos() {
        assert!(qr_svg(&"a".repeat(580)).is_ok());
        assert_eq!(qr_svg(&"a".repeat(581)), Err(QR_TOO_LONG.to_string()));
    }

    #[test]
    fn el_tope_cuenta_bytes_y_no_letras() {
        // 300 letras pero 600 bytes.
        assert_eq!(qr_svg(&"é".repeat(300)), Err(QR_TOO_LONG.to_string()));
    }

    #[test]
    fn una_tarjeta_de_blancos_no_genera_codigo() {
        let card = QrCard {
            name: "  ".into(),
            phones: vec![" ".into()],
            email: Some("".into()),
            organization: "\t".into(),
        };
        assert_eq!(card_svg(&card), Err(QR_EMPTY.to_string()));
    }

    #[test]
    fn una_tarjeta_larga_pero_posible_entra() {
        let card = QrCard {
            name: "María de los Ángeles Pérez García de la Fuente y Rodríguez".into(),
            phones: vec!["+54 9 11 5555-5555".into(), "+54 9 11 4444-4444".into()],
            email: Some("maria.de.los.angeles.perez.garcia@subdominio.ejemplo-largo.com.ar".into()),
            organization: "Fundación Vasak Group para el Desarrollo de Software Libre".into(),
        };
        assert!(card_svg(&card).is_ok());
    }

    /// Con corrección `M`, 580 bytes caen en la versión 19 (93×93 módulos; la 17
    /// aguanta 458). La zona de silencio suma cuatro módulos por lado. Con
    /// corrección más alta el mismo texto pasa de esa versión y deja de leerse
    /// desde la pantalla.
    #[test]
    fn el_codigo_mas_grande_no_pasa_de_la_version_19() {
        let lado = lado_del_svg(&qr_svg(&"a".repeat(580)).unwrap());
        assert!(lado <= 93 + 8, "el código de 580 bytes mide {lado} módulos");
    }

    #[test]
    fn el_codigo_lleva_zona_de_silencio_de_cuatro_modulos() {
        let texto = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Ana\r\nEND:VCARD\r\n";
        let sin_borde = QrCode::with_error_correction_level(texto, EcLevel::M)
            .unwrap()
            .width();
        assert_eq!(lado_del_svg(&qr_svg(texto).unwrap()), sin_borde + 8);
    }

    #[test]
    fn la_correccion_es_m() {
        // Un texto en el que L y M dan versiones distintas, o la prueba no distingue.
        let texto = "a".repeat(300);
        let con = |nivel| {
            QrCode::with_error_correction_level(&texto, nivel)
                .unwrap()
                .width()
        };
        assert_ne!(con(EcLevel::L), con(EcLevel::M));
        assert_eq!(lado_del_svg(&qr_svg(&texto).unwrap()), con(EcLevel::M) + 8);
    }

    #[test]
    fn lo_que_escribe_la_persona_no_llega_al_svg() {
        // El SVG se mete en la ventana con `v-html`: nada del texto puede viajar en él.
        let card = QrCard {
            name: "<script>alert(1)</script><img src=x onerror=alert(1)>".into(),
            ..Default::default()
        };
        let svg = card_svg(&card).unwrap();
        assert!(
            !svg.contains("script") && !svg.contains("onerror") && !svg.contains("<img"),
            "{svg}"
        );
        assert!(!svg.contains("alert"), "{svg}");
    }
}
