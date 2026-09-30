//! Las fechas de una tarjeta: `BDAY`, `ANNIVERSARY` y las fechas con etiqueta
//! de Apple.
//!
//! ── Por qué una fecha puede venir a medias ──────────────────────────────────
//!
//! **Sin año no es un error**: es alguien que no quiere decir cuántos años
//! tiene, y es lo más común en una agenda de verdad. La 4.0 lo escribe
//! `--0415`; Google exporta `--04-15`; y Apple, que en la 3.0 no tiene cómo,
//! inventa el año 1604 y avisa con `X-APPLE-OMIT-YEAR=1604`. Mostrar «15 de
//! abril de 1604» es el error que se ve; tratar la fecha como inválida y no
//! mostrarla es el que no se ve.
//!
//! La 4.0 también deja una fecha sin día (`1985-04`), sólo con el año (`1985`),
//! o sólo con el día (`---15`); y la 2.1 y el `VALUE=text` de la 4.0 dejan
//! **texto libre** («circa 1800»). Lo que no se puede leer como fecha se
//! muestra como vino, en vez de perderse.
//!
//! Cómo se escribe la fecha para la persona lo decide la ventana, con el idioma
//! de la sesión: acá sólo se separan las partes.

use serde::{Deserialize, Serialize};

use super::lines::{display_value, Property};

/// Una fecha que puede venir a medias.
///
/// Las partes que faltan van en `None`. Si la fecha no se pudo leer, las tres
/// van en `None` y `text` trae lo que decía la tarjeta.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialDate {
    pub year: Option<i32>,
    pub month: Option<u8>,
    pub day: Option<u8>,
    /// Lo que decía la tarjeta cuando no era una fecha: vacío si se pudo leer.
    #[serde(default)]
    pub text: String,
}

/// Una fecha con su etiqueta: «aniversario», o la que haya escrito la persona.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabeledDate {
    pub label: String,
    pub date: PartialDate,
}

impl PartialDate {
    fn text(value: &str) -> Option<PartialDate> {
        let value = value.trim();
        (!value.is_empty()).then(|| PartialDate {
            text: value.to_string(),
            ..Default::default()
        })
    }
}

/// La fecha de una propiedad, o `None` si está vacía.
///
/// `clip` recorta el texto libre: una fecha de diez mil caracteres no es una
/// fecha.
pub fn date_from(property: &Property, clip: impl Fn(&str) -> String) -> Option<PartialDate> {
    let value = display_value(property);
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    // `VALUE=text` dice que no es una fecha, y se le cree: «circa 1800» no se
    // puede leer, y un «1985» que la persona escribió como texto tampoco hay
    // que reinterpretarlo.
    if property
        .param("value")
        .is_some_and(|v| v.eq_ignore_ascii_case("text"))
    {
        return PartialDate::text(&clip(value));
    }

    let Some(mut date) = parse_date(value) else {
        return PartialDate::text(&clip(value));
    };

    // El año inventado de Apple: si coincide con el que avisa que hay que
    // omitir, no es un año.
    if let (Some(year), Some(omit)) = (date.year, property.param("x-apple-omit-year")) {
        if omit.trim().parse::<i32>().ok() == Some(year) {
            date.year = None;
        }
    }

    Some(date)
}

/// Lee las formas de fecha de la 3.0 y la 4.0.
///
/// `YYYYMMDD`, `YYYY-MM-DD`, `YYYY-MM`, `YYYY`, `--MMDD`, `--MM-DD`, `--MM` y
/// `---DD`, con o sin una hora pegada detrás de una `T` (`1985-04-15T00:00:00Z`,
/// que es como exportan algunos servidores). La hora se descarta: un cumpleaños
/// no la tiene, y mostrarla en otra zona horaria mueve el día.
///
/// `None` si no es ninguna de esas, o si la fecha no existe: un 31 de febrero
/// no se muestra como fecha, se muestra como vino.
pub fn parse_date(value: &str) -> Option<PartialDate> {
    let date_part = match value.split_once(['T', 't']) {
        Some((date, _time)) if !date.is_empty() => date,
        Some(_) => return None,
        None => value,
    };
    let date_part = date_part.trim();

    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let number = |s: &str| s.parse::<u32>().ok();

    let (year, month, day) = if let Some(day) = date_part.strip_prefix("---") {
        // `---DD`: sólo el día.
        if !(digits(day) && day.len() == 2) {
            return None;
        }
        (None, None, number(day))
    } else if let Some(rest) = date_part.strip_prefix("--") {
        // `--MMDD`, `--MM-DD` o `--MM`: sin año.
        let rest = rest.replace('-', "");
        if !digits(&rest) {
            return None;
        }
        match rest.len() {
            2 => (None, number(&rest), None),
            4 => (None, number(&rest[..2]), number(&rest[2..])),
            _ => return None,
        }
    } else {
        let compact = date_part.replace('-', "");
        if !digits(&compact) {
            return None;
        }
        let dashed = date_part.contains('-');
        match (compact.len(), dashed) {
            (4, false) => (number(&compact).map(|y| y as i32), None, None),
            // `YYYY-MM` sólo con guion: `YYYYMM` no existe en el estándar
            // porque se confunde con otras formas.
            (6, true) => (
                number(&compact[..4]).map(|y| y as i32),
                number(&compact[4..]),
                None,
            ),
            (8, _) => (
                number(&compact[..4]).map(|y| y as i32),
                number(&compact[4..6]),
                number(&compact[6..]),
            ),
            _ => return None,
        }
    };

    let month = month.map(u8::try_from).transpose().ok()?;
    let day = day.map(u8::try_from).transpose().ok()?;

    if let Some(m) = month {
        if !(1..=12).contains(&m) {
            return None;
        }
    }
    if let Some(d) = day {
        // Sin mes se acepta hasta 31; con mes, lo que tenga ese mes. Sin año,
        // el 29 de febrero vale: puede ser el de alguien que nació en uno
        // bisiesto.
        let max = month.map_or(31, |m| days_in_month(year, m));
        if d == 0 || d > max {
            return None;
        }
    }
    // Un «año 0» es lo que escriben algunos exportadores para decir «sin año».
    let year = year.filter(|y| *y != 0);

    Some(PartialDate {
        year,
        month,
        day,
        text: String::new(),
    })
}

fn days_in_month(year: Option<i32>, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        // Sin año, se deja pasar el 29: no hay cómo saber que no era bisiesto.
        _ => match year {
            Some(y) if !(y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)) => 28,
            _ => 29,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vcard::lines::split_property;

    fn date(line: &str) -> Option<PartialDate> {
        date_from(&split_property(line).unwrap(), |s| s.to_string())
    }

    fn ymd(year: Option<i32>, month: Option<u8>, day: Option<u8>) -> Option<PartialDate> {
        Some(PartialDate {
            year,
            month,
            day,
            text: String::new(),
        })
    }

    #[test]
    fn se_leen_las_fechas_completas_de_la_30_y_la_40() {
        assert_eq!(date("BDAY:19850415"), ymd(Some(1985), Some(4), Some(15)));
        assert_eq!(date("BDAY:1985-04-15"), ymd(Some(1985), Some(4), Some(15)));
        assert_eq!(
            date("BDAY;VALUE=date:1985-04-15"),
            ymd(Some(1985), Some(4), Some(15))
        );
    }

    /// **Sin año no es un error**: es alguien que no quiere decirlo, y es lo
    /// más común en una agenda de verdad.
    #[test]
    fn una_fecha_sin_anio_se_lee_sin_anio() {
        assert_eq!(date("BDAY:--0415"), ymd(None, Some(4), Some(15)));
        // La forma de Google.
        assert_eq!(date("BDAY:--04-15"), ymd(None, Some(4), Some(15)));
        // Sólo el mes, y sólo el día: las dos existen en la 4.0.
        assert_eq!(date("BDAY:--04"), ymd(None, Some(4), None));
        assert_eq!(date("BDAY:---15"), ymd(None, None, Some(15)));
    }

    /// Apple no tiene cómo escribir una fecha sin año en la 3.0, así que
    /// inventa el 1604 y lo avisa en un parámetro. «15 de abril de 1604» es el
    /// error que se vería.
    #[test]
    fn el_anio_inventado_de_apple_no_se_muestra() {
        assert_eq!(
            date("BDAY;X-APPLE-OMIT-YEAR=1604:1604-04-15"),
            ymd(None, Some(4), Some(15))
        );
        // Pero si el año no coincide con el que avisa, es un año de verdad.
        assert_eq!(
            date("BDAY;X-APPLE-OMIT-YEAR=1604:1985-04-15"),
            ymd(Some(1985), Some(4), Some(15))
        );
    }

    #[test]
    fn las_fechas_sin_dia_o_solo_con_el_anio_tambien() {
        assert_eq!(date("BDAY:1985-04"), ymd(Some(1985), Some(4), None));
        assert_eq!(date("BDAY:1985"), ymd(Some(1985), None, None));
    }

    /// La hora se descarta: un cumpleaños no la tiene, y mostrarla en otra
    /// zona horaria mueve el día.
    #[test]
    fn la_hora_pegada_se_descarta() {
        assert_eq!(
            date("BDAY:1985-04-15T00:00:00Z"),
            ymd(Some(1985), Some(4), Some(15))
        );
        assert_eq!(
            date("ANNIVERSARY:20100612T160000"),
            ymd(Some(2010), Some(6), Some(12))
        );
    }

    /// El 29 de febrero sin año vale —puede ser de alguien que nació en uno
    /// bisiesto—, y con un año que no lo era no es una fecha.
    #[test]
    fn el_29_de_febrero_depende_del_anio() {
        assert_eq!(date("BDAY:--0229"), ymd(None, Some(2), Some(29)));
        assert_eq!(date("BDAY:20000229"), ymd(Some(2000), Some(2), Some(29)));
        assert_eq!(
            date("BDAY:19000229").unwrap().text,
            "19000229",
            "1900 no fue bisiesto"
        );
    }

    /// Lo que no es una fecha **se muestra como vino**, en vez de perderse: la
    /// 2.1 deja texto libre, y la 4.0 lo dice con `VALUE=text`.
    #[test]
    fn lo_que_no_es_una_fecha_se_conserva_como_texto() {
        assert_eq!(date("BDAY:circa 1800").unwrap().text, "circa 1800");
        assert_eq!(
            date("BDAY;VALUE=text:el día que llegó").unwrap().text,
            "el día que llegó"
        );
        // Y un «1985» escrito como texto no se reinterpreta.
        assert_eq!(date("BDAY;VALUE=text:1985").unwrap().text, "1985");
        // Fechas imposibles: tampoco se inventa una.
        for imposible in [
            "BDAY:19851345",
            "BDAY:--1301",
            "BDAY:1985-02-30",
            "BDAY:198504",
        ] {
            let leida = date(imposible).unwrap();
            assert!(leida.year.is_none() && leida.month.is_none(), "{imposible}");
            assert!(!leida.text.is_empty(), "{imposible}");
        }
    }

    #[test]
    fn una_fecha_vacia_no_es_nada() {
        assert_eq!(date("BDAY:"), None);
        assert_eq!(date("BDAY:   "), None);
    }

    /// Un «año 0» es lo que escriben algunos exportadores para decir que no
    /// hay año.
    #[test]
    fn el_anio_cero_es_sin_anio() {
        assert_eq!(date("BDAY:0000-04-15"), ymd(None, Some(4), Some(15)));
    }

    /// Nada de lo que traiga una tarjeta puede hacer caer la lectura.
    #[test]
    fn lo_roto_no_hace_caer_nada() {
        for basura in [
            "T",
            "--",
            "---",
            "-",
            "T10",
            "9999999999999",
            "--99999",
            "1985-",
            "日本",
            "--0415T",
            "----",
        ] {
            let _ = parse_date(basura);
        }
        assert_eq!(parse_date("T10"), None);
    }
}
