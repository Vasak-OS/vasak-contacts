//! La foto de un contacto.
//!
//! ── Las tres formas en que llega ────────────────────────────────────────────
//!
//! - **Dentro de la tarjeta, en base64**, que es lo de la 2.1 y la 3.0:
//!   `PHOTO;ENCODING=b;TYPE=JPEG:/9j/4AAQ…` (o `ENCODING=BASE64` con el tipo
//!   suelto, en la 2.1). Es lo que manda iCloud y lo que exportan los teléfonos.
//! - **Dentro de la tarjeta, como `data:` URI**, que es lo de la 4.0:
//!   `PHOTO:data:image/jpeg;base64,/9j/4AAQ…`. Nextcloud escribe ésta.
//! - **Como una dirección externa**: `PHOTO;VALUE=uri:https://…`. Google
//!   manda ésta.
//!
//! ── La dirección externa no la carga la ventana ─────────────────────────────
//!
//! **Nunca.** Pedirla desde la ventana es un rastreador de apertura, igual que
//! una imagen remota en un correo: quien controla esa dirección se entera de
//! cuándo se abrió la ficha y desde dónde, con las cabeceras del motor del
//! navegador. La baja el programa, una sola vez, sin cookies ni `Referer`, y la
//! guarda en la caché: ver `photos.rs`. Y la CSP de la ventana tampoco deja
//! cargar imágenes de otro origen que no sea `data:`, así que aunque algo se
//! equivocara acá, el motor no la pediría.
//!
//! ── El tope ─────────────────────────────────────────────────────────────────
//!
//! Una foto en base64 pasa fácil los [`super::contact::MAX_VALUE`] bytes de
//! cualquier otro valor, así que tiene su tope propio, más alto:
//! [`MAX_PHOTO_BYTES`] **decodificados**. Lo que lo pasa no se muestra —y se
//! dice por qué— en vez de mandar un megabyte a la ventana por cada contacto.
//!
//! **Se decodifica una sola vez**, acá, y sólo la primera foto que sirva: para
//! comprobar que es una imagen de verdad (por sus primeros bytes, no por el
//! tipo que declara la tarjeta) y que entra en el tope. La ventana recibe la
//! `data:` URI armada y no vuelve a mirar nada.

use base64::alphabet;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
use base64::engine::DecodePaddingMode;
use base64::Engine;
use serde::{Deserialize, Serialize};

use super::lines::{unescape_text, Property};

/// El tope de una foto, decodificada.
///
/// 256 KiB alcanzan de sobra para lo que se muestra —un círculo de pocos
/// centenares de píxeles— y para lo que mandan los servidores de verdad: Google
/// y iCloud sirven fotos de entre 10 y 100 KiB. Y una libreta de mil contactos
/// con la foto al tope ya son 350 MB de base64 cruzando a la ventana, así que
/// más alto no tiene sentido.
pub const MAX_PHOTO_BYTES: usize = 256 * 1024;

/// Lo que ocupa en base64 una foto al tope: cuatro caracteres por cada tres
/// bytes. Se mira **antes** de decodificar, para no reservar memoria para algo
/// que se va a tirar.
const MAX_PHOTO_BASE64: usize = MAX_PHOTO_BYTES.div_ceil(3) * 4;

/// Base64 que acepta el relleno como venga: hay exportadores que lo omiten y
/// otros que dejan bits sobrantes al final, y la foto se ve igual.
const LENIENT: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true),
);

/// Por qué una tarjeta con foto se muestra con las iniciales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhotoSkipped {
    /// Pasa el tope de [`MAX_PHOTO_BYTES`].
    TooLarge,
    /// No es una imagen que se sepa mostrar sin riesgo: ni JPEG, ni PNG, ni
    /// GIF, ni WebP. Un SVG entra acá: puede traer referencias externas.
    Unsupported,
    /// Una dirección externa que no es `https`: pedirla por `http` la deja ver
    /// y cambiar a cualquiera que esté en el medio.
    Insecure,
}

/// Lo que se sacó de una `PHOTO`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotoSource {
    /// Una `data:` URI lista para mostrar.
    Inline(String),
    /// Una dirección `https` para que el programa la baje y la guarde.
    Remote(String),
    Skipped(PhotoSkipped),
}

/// Los formatos que se muestran, reconocidos por sus primeros bytes.
///
/// **Por los bytes y no por lo que declara la tarjeta**: `TYPE=JPEG` lo
/// escribió cualquiera, y lo que importa es qué hay adentro.
pub fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Arma la `data:` URI de una imagen ya comprobada.
pub fn data_uri(mime: &str, bytes: &[u8]) -> String {
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// La foto de una propiedad `PHOTO`.
pub fn photo_from(property: &Property) -> Option<PhotoSource> {
    let value = property.value.trim();
    if value.is_empty() {
        return None;
    }

    // 2.1 y 3.0: `ENCODING=b` o `ENCODING=BASE64`, o `BASE64` suelto en la 2.1.
    let base64_param = property
        .param("encoding")
        .is_some_and(|e| e.eq_ignore_ascii_case("b") || e.eq_ignore_ascii_case("base64"))
        || property
            .params
            .iter()
            .any(|p| p.trim().eq_ignore_ascii_case("base64"));
    if base64_param {
        return Some(inline_photo(value));
    }

    // 4.0: `data:image/jpeg;base64,…`.
    if value.len() >= 5 && value.is_char_boundary(5) && value[..5].eq_ignore_ascii_case("data:") {
        let Some((header, payload)) = value[5..].split_once(',') else {
            return Some(PhotoSource::Skipped(PhotoSkipped::Unsupported));
        };
        // Sólo base64: una `data:` en porcentajes es de un SVG casi siempre.
        if !header.to_ascii_lowercase().ends_with(";base64") {
            return Some(PhotoSource::Skipped(PhotoSkipped::Unsupported));
        }
        return Some(inline_photo(payload));
    }

    // Una dirección externa. Desescapada: Google exporta `https\://…`, con
    // los dos puntos escapados como si fuera texto.
    let value = unescape_text(value);
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("https://") {
        // Una dirección con espacios o controles no es una dirección.
        if value.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Some(PhotoSource::Skipped(PhotoSkipped::Unsupported));
        }
        return Some(PhotoSource::Remote(value.to_string()));
    }
    if lower.starts_with("http://") {
        return Some(PhotoSource::Skipped(PhotoSkipped::Insecure));
    }

    // Cualquier otro esquema —`file:`, `cid:`, `ftp:`— o algo que no se sabe
    // leer.
    Some(PhotoSource::Skipped(PhotoSkipped::Unsupported))
}

/// Decodifica una foto en base64 **una vez**, y la devuelve lista para mostrar.
fn inline_photo(payload: &str) -> PhotoSource {
    // El tope se mira antes de sacar los espacios: un valor con megas de
    // espacios adentro no es una foto, y copiarlo para limpiarlo ya es gastar.
    // El margen de un cuarto es para los saltos y los espacios que dejan los
    // exportadores de la 2.1 entre renglón y renglón.
    if payload.len() > MAX_PHOTO_BASE64 + MAX_PHOTO_BASE64 / 4 {
        return PhotoSource::Skipped(PhotoSkipped::TooLarge);
    }
    let clean: String = payload
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    if clean.len() > MAX_PHOTO_BASE64 {
        return PhotoSource::Skipped(PhotoSkipped::TooLarge);
    }

    let Ok(bytes) = LENIENT.decode(clean.as_bytes()) else {
        return PhotoSource::Skipped(PhotoSkipped::Unsupported);
    };
    if bytes.len() > MAX_PHOTO_BYTES {
        return PhotoSource::Skipped(PhotoSkipped::TooLarge);
    }
    match sniff_image(&bytes) {
        Some(mime) => PhotoSource::Inline(data_uri(mime, &bytes)),
        None => PhotoSource::Skipped(PhotoSkipped::Unsupported),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::vcard::lines::split_property;

    /// Un PNG de un píxel, de verdad: lo que interesa es que empiece como
    /// empieza un PNG.
    pub(crate) const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    /// El comienzo de un JPEG, con relleno: alcanza para reconocerlo.
    pub(crate) fn jpeg_of(len: usize) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0];
        bytes.resize(len, 0x42);
        bytes
    }

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    fn photo(line: &str) -> Option<PhotoSource> {
        photo_from(&split_property(line).unwrap())
    }

    /// La de la 3.0, que manda iCloud.
    #[test]
    fn se_lee_la_foto_en_base64_de_la_30() {
        let line = format!("PHOTO;ENCODING=b;TYPE=JPEG:{}", b64(&jpeg_of(64)));
        let Some(PhotoSource::Inline(uri)) = photo(&line) else {
            panic!("no se leyó la foto");
        };
        assert!(uri.starts_with("data:image/jpeg;base64,/9j/"), "{uri}");
    }

    /// La de la 2.1, con el tipo suelto y `BASE64` en vez de `b`.
    #[test]
    fn se_lee_la_foto_de_la_21() {
        let line = format!("PHOTO;JPEG;ENCODING=BASE64:{}", b64(&jpeg_of(64)));
        assert!(matches!(photo(&line), Some(PhotoSource::Inline(_))));
        let suelto = format!("PHOTO;PNG;BASE64:{}", b64(PNG_1X1));
        assert!(
            matches!(photo(&suelto), Some(PhotoSource::Inline(u)) if u.starts_with("data:image/png"))
        );
    }

    /// La de la 4.0, como `data:` URI.
    #[test]
    fn se_lee_la_foto_como_data_uri_de_la_40() {
        let line = format!("PHOTO:data:image/png;base64,{}", b64(PNG_1X1));
        let Some(PhotoSource::Inline(uri)) = photo(&line) else {
            panic!("no se leyó la foto");
        };
        assert_eq!(uri, format!("data:image/png;base64,{}", b64(PNG_1X1)));
    }

    /// **El tipo lo dicen los bytes, no la tarjeta.** Un PNG declarado como
    /// JPEG se muestra como PNG; algo que no es una imagen no se muestra.
    #[test]
    fn el_tipo_lo_dicen_los_bytes() {
        let mentira = format!("PHOTO;ENCODING=b;TYPE=JPEG:{}", b64(PNG_1X1));
        assert!(
            matches!(photo(&mentira), Some(PhotoSource::Inline(u)) if u.starts_with("data:image/png"))
        );

        let texto = format!(
            "PHOTO;ENCODING=b;TYPE=JPEG:{}",
            b64(b"<html>no soy una foto")
        );
        assert_eq!(
            photo(&texto),
            Some(PhotoSource::Skipped(PhotoSkipped::Unsupported))
        );
    }

    /// Un SVG no entra: puede traer referencias externas, que es justo lo que
    /// la foto remota no puede hacer.
    #[test]
    fn un_svg_no_se_muestra() {
        let svg = format!(
            "PHOTO:data:image/svg+xml;base64,{}",
            b64(b"<svg xmlns='http://www.w3.org/2000/svg'><image href='https://x/t.gif'/></svg>")
        );
        assert_eq!(
            photo(&svg),
            Some(PhotoSource::Skipped(PhotoSkipped::Unsupported))
        );
        let en_porcentajes = "PHOTO:data:image/svg+xml,%3Csvg%3E";
        assert_eq!(
            photo(en_porcentajes),
            Some(PhotoSource::Skipped(PhotoSkipped::Unsupported))
        );
    }

    /// **Una foto enorme no se decodifica**: pasa el tope y se dice por qué.
    #[test]
    fn una_foto_enorme_supera_el_tope() {
        let enorme = format!(
            "PHOTO;ENCODING=b;TYPE=JPEG:{}",
            b64(&jpeg_of(MAX_PHOTO_BYTES + 1))
        );
        assert_eq!(
            photo(&enorme),
            Some(PhotoSource::Skipped(PhotoSkipped::TooLarge))
        );
        // Y una al tope justo, sí.
        let justa = format!(
            "PHOTO;ENCODING=b;TYPE=JPEG:{}",
            b64(&jpeg_of(MAX_PHOTO_BYTES))
        );
        assert!(matches!(photo(&justa), Some(PhotoSource::Inline(_))));
    }

    /// El tope de la foto es **más alto** que el de cualquier otro valor, que
    /// es lo que la deja pasar.
    #[test]
    fn el_tope_de_la_foto_es_mas_alto_que_el_de_un_valor() {
        const { assert!(MAX_PHOTO_BYTES > crate::vcard::contact::MAX_VALUE) };
        let mediana = format!(
            "PHOTO;ENCODING=b;TYPE=JPEG:{}",
            b64(&jpeg_of(crate::vcard::contact::MAX_VALUE * 4))
        );
        assert!(matches!(photo(&mediana), Some(PhotoSource::Inline(_))));
    }

    /// Los espacios y los saltos que dejan los exportadores de la 2.1 entre
    /// renglón y renglón no rompen la foto.
    #[test]
    fn los_espacios_adentro_del_base64_no_la_rompen() {
        let codificada = b64(&jpeg_of(300));
        let con_espacios: String = codificada
            .as_bytes()
            .chunks(20)
            .map(|c| std::str::from_utf8(c).unwrap())
            .collect::<Vec<_>>()
            .join("  ");
        let line = format!("PHOTO;ENCODING=b:{con_espacios}");
        assert!(matches!(photo(&line), Some(PhotoSource::Inline(_))));

        // Y sin el relleno final también.
        let sin_relleno = format!("PHOTO;ENCODING=b:{}", codificada.trim_end_matches('='));
        assert!(matches!(photo(&sin_relleno), Some(PhotoSource::Inline(_))));
    }

    /// **La dirección externa no se carga acá**: se entrega para que el
    /// programa la baje, y sólo si es `https`.
    #[test]
    fn una_url_externa_no_se_descarga_al_leer() {
        assert_eq!(
            photo("PHOTO;VALUE=uri:https://lh3.googleusercontent.com/contacts/abc"),
            Some(PhotoSource::Remote(
                "https://lh3.googleusercontent.com/contacts/abc".into()
            ))
        );
        // Google escapa los dos puntos.
        assert_eq!(
            photo(r"PHOTO:https\://ejemplo.com/ana.jpg"),
            Some(PhotoSource::Remote("https://ejemplo.com/ana.jpg".into()))
        );
        // `http` no: la vería y la cambiaría cualquiera en el medio.
        assert_eq!(
            photo("PHOTO;VALUE=uri:http://ejemplo.com/ana.jpg"),
            Some(PhotoSource::Skipped(PhotoSkipped::Insecure))
        );
        // Ni otro esquema.
        for otra in [
            "PHOTO;VALUE=uri:file:///etc/passwd",
            "PHOTO:ftp://ejemplo.com/a.jpg",
            "PHOTO:https://ejemplo.com/a b.jpg",
        ] {
            assert_eq!(
                photo(otra),
                Some(PhotoSource::Skipped(PhotoSkipped::Unsupported)),
                "{otra}"
            );
        }
    }

    #[test]
    fn lo_roto_no_hace_caer_nada() {
        for basura in [
            "PHOTO:",
            "PHOTO;ENCODING=b:",
            "PHOTO;ENCODING=b:!!!!",
            "PHOTO:data:",
            "PHOTO:data:image/png;base64,",
            "PHOTO:dató",
            "PHOTO:日本",
        ] {
            let _ = photo(basura);
        }
        assert_eq!(photo("PHOTO:"), None);
    }
}
