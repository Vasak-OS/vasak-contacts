//! Las redes y la mensajería: `IMPP`, `X-SOCIALPROFILE` y los `X-` de cada
//! servicio.
//!
//! ── Por qué hay tantas formas ───────────────────────────────────────────────
//!
//! El estándar tiene `IMPP` para la mensajería (`xmpp:`, `sip:`, `skype:`) y
//! nada para las redes, así que cada uno inventó la suya:
//!
//! - **Apple** y **Nextcloud**: `X-SOCIALPROFILE;TYPE=twitter:…`. Apple pone la
//!   dirección del perfil en el valor y el usuario en `X-USER`; Nextcloud pone
//!   sólo el usuario.
//! - **Los exportadores viejos** —Evolution, KDE, Thunderbird, los teléfonos—:
//!   una propiedad por servicio, `X-SKYPE`, `X-JABBER`, `X-TWITTER`, `X-ICQ`…
//! - **`IMPP`** dice el servicio por el esquema (`xmpp:`), o por
//!   `X-SERVICE-TYPE` cuando el esquema no lo dice (`x-apple:`).
//!
//! Se leen todas a la misma forma: qué servicio, qué usuario, y la dirección
//! del perfil **sólo si es `https`** o `http` —que es lo único que la ventana
//! abre—. Una dirección `javascript:` o `file:` en un perfil no se abre nunca.

use serde::{Deserialize, Serialize};

use super::lines::{display_value, Property};

/// Una cuenta en una red o en un servicio de mensajería.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialProfile {
    /// El servicio, en minúsculas: «twitter», «skype», «xmpp». Vacío si la
    /// tarjeta no lo dice.
    pub service: String,
    /// La etiqueta: «casa», «trabajo», o la de Apple.
    #[serde(default)]
    pub label: String,
    /// El usuario, o la dirección entera si es lo único que hay.
    pub handle: String,
    /// La dirección del perfil, sólo si es `http` o `https`. Vacía si no hay
    /// una que se pueda abrir.
    #[serde(default)]
    pub url: String,
}

/// Los `X-` de un servicio, y a qué servicio corresponden.
///
/// Los nombres son los que escriben Evolution, KDE, Thunderbird y los
/// exportadores de teléfonos. `X-GOOGLE-TALK` y `X-GTALK` son el mismo.
const SERVICE_PROPERTIES: &[(&str, &str)] = &[
    ("X-AIM", "aim"),
    ("X-FACEBOOK", "facebook"),
    ("X-GADUGADU", "gadugadu"),
    ("X-GOOGLE-TALK", "gtalk"),
    ("X-GROUPWISE", "groupwise"),
    ("X-GTALK", "gtalk"),
    ("X-ICQ", "icq"),
    ("X-INSTAGRAM", "instagram"),
    ("X-JABBER", "xmpp"),
    ("X-LINKEDIN", "linkedin"),
    ("X-MASTODON", "mastodon"),
    ("X-MATRIX", "matrix"),
    ("X-MSN", "msn"),
    ("X-QQ", "qq"),
    ("X-SIGNAL", "signal"),
    ("X-SKYPE", "skype"),
    ("X-SKYPE-USERNAME", "skype"),
    ("X-TELEGRAM", "telegram"),
    ("X-TWITTER", "twitter"),
    ("X-WHATSAPP", "whatsapp"),
    ("X-YAHOO", "yahoo"),
];

/// Los esquemas de `IMPP` y a qué servicio corresponden.
const IMPP_SCHEMES: &[(&str, &str)] = &[
    ("aim", "aim"),
    ("gg", "gadugadu"),
    ("gtalk", "gtalk"),
    ("icq", "icq"),
    ("irc", "irc"),
    ("matrix", "matrix"),
    ("msnim", "msn"),
    ("qq", "qq"),
    ("sip", "sip"),
    ("sips", "sip"),
    ("skype", "skype"),
    ("xmpp", "xmpp"),
    ("ymsgr", "yahoo"),
];

/// Si una propiedad es de las que se leen acá.
pub fn is_social(name: &str) -> bool {
    name == "IMPP"
        || name == "X-SOCIALPROFILE"
        || SERVICE_PROPERTIES.iter().any(|(n, _)| *n == name)
}

/// La dirección, si es una que la ventana puede abrir: `http` o `https`, sin
/// espacios ni controles.
pub fn web_url(value: &str) -> Option<String> {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    let scheme_ok = lower.starts_with("https://") || lower.starts_with("http://");
    let clean = !value.chars().any(|c| c.is_whitespace() || c.is_control());
    // Algo después del esquema: `https://` solo no es una dirección.
    let has_host = value
        .split_once("://")
        .is_some_and(|(_, rest)| !rest.is_empty());
    (scheme_ok && clean && has_host).then(|| value.to_string())
}

/// Lee un perfil de una propiedad, o `None` si está vacía.
pub fn social_from(property: &Property, label: String) -> Option<SocialProfile> {
    let value = display_value(property);
    let value = value.trim();

    let declared = property
        .param("x-service-type")
        .or_else(|| {
            property
                .param("type")
                .filter(|_| property.name == "X-SOCIALPROFILE")
        })
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    let profile = match property.name.as_str() {
        "X-SOCIALPROFILE" => {
            // Apple: la dirección en el valor y el usuario en `X-USER`.
            let url = web_url(value).unwrap_or_default();
            let handle = property
                .param("x-user")
                .map(str::to_string)
                .filter(|h| !h.trim().is_empty())
                .unwrap_or_else(|| value.to_string());
            SocialProfile {
                service: declared,
                label: String::new(),
                handle,
                url,
            }
        }
        "IMPP" => {
            let (scheme, rest) = value.split_once(':').unwrap_or(("", value));
            let scheme = scheme.to_ascii_lowercase();
            let by_scheme = IMPP_SCHEMES
                .iter()
                .find(|(s, _)| *s == scheme)
                .map(|(_, service)| service.to_string());
            let service = if declared.is_empty() {
                by_scheme.unwrap_or_default()
            } else {
                declared
            };
            SocialProfile {
                service,
                label: label.clone(),
                // `xmpp:ana@x` se muestra `ana@x`; el esquema no le dice nada a
                // nadie.
                handle: if scheme.is_empty() { value } else { rest }.to_string(),
                url: String::new(),
            }
        }
        name => {
            let service = SERVICE_PROPERTIES
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, s)| s.to_string())?;
            SocialProfile {
                service,
                label: label.clone(),
                handle: value.to_string(),
                url: web_url(value).unwrap_or_default(),
            }
        }
    };

    let profile = SocialProfile {
        label: if profile.label.is_empty() {
            label
        } else {
            profile.label
        },
        ..profile
    };
    (!profile.handle.trim().is_empty()).then_some(profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vcard::lines::split_property;

    fn social(line: &str) -> Option<SocialProfile> {
        social_from(&split_property(line).unwrap(), String::new())
    }

    /// La forma de Apple: la dirección del perfil en el valor y el usuario en
    /// un parámetro.
    #[test]
    fn se_lee_el_perfil_de_apple() {
        let p = social("X-SOCIALPROFILE;TYPE=twitter;X-USER=anaperez:https://twitter.com/anaperez")
            .unwrap();
        assert_eq!(p.service, "twitter");
        assert_eq!(p.handle, "anaperez");
        assert_eq!(p.url, "https://twitter.com/anaperez");
    }

    /// La de Nextcloud: sólo el usuario, sin dirección que abrir.
    #[test]
    fn se_lee_el_perfil_de_nextcloud() {
        let p = social("X-SOCIALPROFILE;TYPE=mastodon:@ana@mastodon.social").unwrap();
        assert_eq!(p.service, "mastodon");
        assert_eq!(p.handle, "@ana@mastodon.social");
        assert_eq!(p.url, "");
    }

    /// Los `X-` de cada servicio, que escriben Evolution, KDE y los teléfonos.
    #[test]
    fn se_leen_los_x_de_cada_servicio() {
        for (line, service, handle) in [
            ("X-SKYPE:ana.perez", "skype", "ana.perez"),
            ("X-SKYPE-USERNAME:ana.perez", "skype", "ana.perez"),
            ("X-TWITTER:@anaperez", "twitter", "@anaperez"),
            (
                "X-JABBER;TYPE=HOME:ana@jabber.org",
                "xmpp",
                "ana@jabber.org",
            ),
            ("X-ICQ:123456", "icq", "123456"),
            ("X-GOOGLE-TALK:ana@gmail.com", "gtalk", "ana@gmail.com"),
        ] {
            let p = social(line).unwrap_or_else(|| panic!("{line}"));
            assert_eq!(p.service, service, "{line}");
            assert_eq!(p.handle, handle, "{line}");
        }
    }

    /// `IMPP` dice el servicio por el esquema, o por `X-SERVICE-TYPE` cuando el
    /// esquema no lo dice.
    #[test]
    fn se_lee_impp_por_el_esquema_o_por_el_parametro() {
        let xmpp = social("IMPP:xmpp:ana@jabber.org").unwrap();
        assert_eq!(
            (xmpp.service.as_str(), xmpp.handle.as_str()),
            ("xmpp", "ana@jabber.org")
        );

        let apple = social("IMPP;X-SERVICE-TYPE=Skype;TYPE=HOME:x-apple:ana.perez").unwrap();
        assert_eq!(
            (apple.service.as_str(), apple.handle.as_str()),
            ("skype", "ana.perez")
        );

        let sip = social("IMPP;TYPE=work:sip:ana@voip.ejemplo.com").unwrap();
        assert_eq!(sip.service, "sip");
        // `IMPP` no tiene dirección que abrir: no es una web.
        assert_eq!(sip.url, "");
    }

    /// **Una dirección que no es web no se abre nunca**: el perfil la muestra
    /// como texto.
    #[test]
    fn una_direccion_que_no_es_web_no_se_ofrece_para_abrir() {
        for line in [
            "X-SOCIALPROFILE;TYPE=twitter:javascript:alert(1)",
            "X-SOCIALPROFILE;TYPE=twitter:file:///etc/passwd",
            "X-SOCIALPROFILE;TYPE=twitter:https://x.com/a b",
            "X-SOCIALPROFILE;TYPE=twitter:https://",
        ] {
            assert_eq!(social(line).unwrap().url, "", "{line}");
        }
        assert_eq!(
            web_url("http://ejemplo.com").as_deref(),
            Some("http://ejemplo.com")
        );
    }

    #[test]
    fn un_perfil_vacio_no_es_un_perfil() {
        assert_eq!(social("X-SKYPE:"), None);
        assert_eq!(social("IMPP:"), None);
        assert_eq!(social("X-SOCIALPROFILE;TYPE=twitter:"), None);
        // Y lo que no es de acá, tampoco.
        assert_eq!(social("X-OTRA-COSA:valor"), None);
        assert!(!is_social("X-OTRA-COSA"));
        assert!(is_social("IMPP"));
    }
}
