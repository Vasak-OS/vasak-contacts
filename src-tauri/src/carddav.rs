//! Leer los contactos de un servidor CardDAV.
//!
//! Dos pasos, como en el calendario: preguntar qué libretas tiene la persona
//! (`PROPFIND` sobre su carpeta) y pedir las tarjetas de una (`REPORT`). El
//! servidor devuelve vCard, que se interpreta en `vcard.rs`.
//!
//! ── Por qué se traen todas de una ───────────────────────────────────────────
//!
//! El calendario pide los eventos de un mes porque un calendario **se mira por
//! mes**. Una libreta no: se busca a alguien, y para buscar hay que tener todo.
//! Una agenda de mil contactos son unos pocos megabytes de texto, se trae una
//! vez al abrir la aplicación y se busca en memoria — que es instantáneo y no
//! le pega al servidor con cada tecla.
//!
//! ── Lo que **no** hace todavía ──────────────────────────────────────────────
//!
//! No crea, no edita y no borra. Y no muestra las fotos: una `PHOTO` en base64
//! multiplica por diez el tamaño de la respuesta, y traerla para una lista donde
//! no se ve sería gastar la conexión de la persona en nada.

use std::time::Duration;

use base64::Engine;
use serde::Serialize;

use crate::vcard::{self, Contacto};

const TIMEOUT: Duration = Duration::from_secs(30);

/// Tope de lo que se lee de una respuesta.
///
/// Una agenda de mil contactos son unos pocos megabytes. Dieciséis es de sobra
/// y corta un servidor que devuelve basura antes de que la memoria de la
/// ventana crezca sin freno.
const MAX_CUERPO: usize = 16 * 1024 * 1024;

const NS_DAV: &str = "DAV:";
const NS_CARDDAV: &str = "urn:ietf:params:xml:ns:carddav";

/// Una libreta de la persona.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Libreta {
    pub url: String,
    pub nombre: String,
}

// ---------------------------------------------------------------------------
// Lo que se puede probar sin red
// ---------------------------------------------------------------------------

/// Lee las libretas de una respuesta `PROPFIND`.
pub fn libretas_de(xml: &str, base: &str) -> Vec<Libreta> {
    let Ok(documento) = roxmltree::Document::parse(xml) else {
        return Vec::new();
    };

    documento
        .descendants()
        .filter(|n| n.has_tag_name((NS_DAV, "response")))
        .filter_map(|respuesta| {
            // Sólo las colecciones que de verdad son libretas: la carpeta trae
            // también cosas que no lo son, y listarlas daría entradas que al
            // abrirlas no tienen nada.
            let es_libreta = respuesta
                .descendants()
                .any(|n| n.has_tag_name((NS_CARDDAV, "addressbook")));
            if !es_libreta {
                return None;
            }

            let href = respuesta
                .descendants()
                .find(|n| n.has_tag_name((NS_DAV, "href")))?
                .text()?
                .trim();
            let url = reqwest::Url::parse(base).ok()?.join(href).ok()?.to_string();

            let nombre = respuesta
                .descendants()
                .find(|n| n.has_tag_name((NS_DAV, "displayname")))
                .and_then(|n| n.text())
                .unwrap_or("")
                .trim()
                .to_string();

            Some(Libreta {
                url,
                // Una libreta sin nombre igual se muestra: es donde puede estar
                // el contacto que la persona busca.
                nombre: if nombre.is_empty() { "Contactos".into() } else { nombre },
            })
        })
        .collect()
}

/// Saca las tarjetas y su dirección de una respuesta `REPORT`.
///
/// La dirección va con la tarjeta porque es lo que la identifica en el
/// servidor: el `UID` de adentro lo escribe quien la creó y puede faltar, estar
/// repetido, o ser el mismo en dos libretas distintas.
pub fn tarjetas_de(xml: &str, base: &str) -> Vec<(String, String)> {
    let Ok(documento) = roxmltree::Document::parse(xml) else {
        return Vec::new();
    };

    documento
        .descendants()
        .filter(|n| n.has_tag_name((NS_DAV, "response")))
        .filter_map(|respuesta| {
            let datos = respuesta
                .descendants()
                .find(|n| n.has_tag_name((NS_CARDDAV, "address-data")))?
                .text()?;

            let href = respuesta
                .descendants()
                .find(|n| n.has_tag_name((NS_DAV, "href")))
                .and_then(|n| n.text())
                .unwrap_or("")
                .trim();
            let url = reqwest::Url::parse(base)
                .ok()
                .and_then(|b| b.join(href).ok())
                .map(|u| u.to_string())
                .unwrap_or_default();

            Some((url, datos.to_string()))
        })
        .collect()
}

/// El cuerpo del `PROPFIND` que pide las libretas.
pub fn consulta_de_libretas() -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="{NS_CARDDAV}">
  <d:prop><d:resourcetype/><d:displayname/></d:prop>
</d:propfind>"#
    )
}

/// El cuerpo del `REPORT` que pide todas las tarjetas de una libreta.
pub fn consulta_de_tarjetas() -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<c:addressbook-query xmlns:d="DAV:" xmlns:c="{NS_CARDDAV}">
  <d:prop><d:getetag/><c:address-data/></d:prop>
</c:addressbook-query>"#
    )
}

// ---------------------------------------------------------------------------
// La parte que habla por la red
// ---------------------------------------------------------------------------

fn cabecera_basica(usuario: &str, secreto: &str) -> String {
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{usuario}:{secreto}"))
    )
}

fn cliente() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        // Sin redirecciones: el pedido lleva la contraseña, y una redirección la
        // mandaría adonde el servidor diga.
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("VasakOS")
        .build()
        .map_err(|e| format!("no se pudo crear el cliente HTTP: {e}"))
}

async fn cuerpo_con_tope(mut respuesta: reqwest::Response) -> Result<String, String> {
    let estado = respuesta.status();
    if estado == reqwest::StatusCode::UNAUTHORIZED {
        return Err("el servidor rechazó el usuario o la contraseña. \
                    Volvé a conectar la cuenta desde Configuración"
            .into());
    }
    if !estado.is_success() {
        return Err(format!("el servidor respondió {estado}"));
    }

    // Por trozos y cortando en el momento: leer todo y medir después es
    // enterarse del problema cuando ya pasó — un servidor que manda gigabytes
    // hace crecer la memoria de la ventana hasta donde quiera.
    let mut cuerpo = Vec::new();
    while let Some(trozo) = respuesta
        .chunk()
        .await
        .map_err(|e| format!("no se pudo leer la respuesta: {e}"))?
    {
        if cuerpo.len() + trozo.len() > MAX_CUERPO {
            return Err(format!(
                "el servidor mandó más de {MAX_CUERPO} bytes, que es lo que se lee de una vez"
            ));
        }
        cuerpo.extend_from_slice(&trozo);
    }

    Ok(String::from_utf8_lossy(&cuerpo).into_owned())
}

/// Las libretas que hay en la carpeta de la persona.
pub async fn libretas(credencial: &crate::cuentas::Credencial) -> Result<Vec<Libreta>, String> {
    let respuesta = cliente()?
        .request(metodo("PROPFIND"), &credencial.home)
        .header(
            "Authorization",
            cabecera_basica(&credencial.usuario, &credencial.secreto),
        )
        // 1: la carpeta y lo que hay dentro. Con 0 sólo vendría la carpeta, que
        // es justo lo que no interesa.
        .header("Depth", "1")
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(consulta_de_libretas())
        .send()
        .await
        .map_err(|e| format!("no se pudo consultar {}: {e}", credencial.home))?;

    let xml = cuerpo_con_tope(respuesta).await?;
    Ok(libretas_de(&xml, &credencial.home))
}

/// Todos los contactos de una libreta.
pub async fn contactos(
    credencial: &crate::cuentas::Credencial,
    libreta: &str,
) -> Result<Vec<Contacto>, String> {
    let respuesta = cliente()?
        .request(metodo("REPORT"), libreta)
        .header(
            "Authorization",
            cabecera_basica(&credencial.usuario, &credencial.secreto),
        )
        // 1: las tarjetas de esta libreta. El estándar lo pide, y hay
        // servidores que sin esto devuelven vacío.
        .header("Depth", "1")
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(consulta_de_tarjetas())
        .send()
        .await
        .map_err(|e| format!("no se pudieron pedir los contactos: {e}"))?;

    let xml = cuerpo_con_tope(respuesta).await?;

    Ok(tarjetas_de(&xml, libreta)
        .into_iter()
        // Una respuesta puede traer varias tarjetas en el mismo bloque: hay
        // libretas exportadas que son un solo archivo con miles.
        .flat_map(|(url, datos)| {
            vcard::tarjetas_de(&datos)
                .into_iter()
                .filter_map(move |t| vcard::contacto_de(&t, &url))
        })
        .collect())
}

fn metodo(nombre: &str) -> reqwest::Method {
    reqwest::Method::from_bytes(nombre.as_bytes()).expect("es un método válido")
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBRETAS: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
  <d:response>
    <d:href>/dav/addressbooks/users/ana/</d:href>
    <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/dav/addressbooks/users/ana/personal/</d:href>
    <d:propstat><d:prop>
      <d:resourcetype><d:collection/><c:addressbook/></d:resourcetype>
      <d:displayname>Personal</d:displayname>
    </d:prop></d:propstat>
  </d:response>
</d:multistatus>"#;

    /// La carpeta trae también cosas que no son libretas. Listarlas daría
    /// entradas que al abrirlas no tienen nada.
    #[test]
    fn solo_se_listan_las_colecciones_que_son_libretas() {
        let libretas = libretas_de(LIBRETAS, "https://nube.ejemplo.com/dav/addressbooks/users/ana/");

        assert_eq!(libretas.len(), 1);
        assert_eq!(libretas[0].nombre, "Personal");
        assert_eq!(
            libretas[0].url,
            "https://nube.ejemplo.com/dav/addressbooks/users/ana/personal/"
        );
    }

    /// Los servidores contestan con una ruta absoluta casi siempre y con una
    /// URL entera a veces. Pegarlas a mano rompería la segunda.
    #[test]
    fn un_href_con_url_entera_no_se_pega_dos_veces() {
        let xml = LIBRETAS.replace(
            "<d:href>/dav/addressbooks/users/ana/personal/</d:href>",
            "<d:href>https://otra.ejemplo.com/x/</d:href>",
        );
        let libretas = libretas_de(&xml, "https://nube.ejemplo.com/dav/addressbooks/users/ana/");
        assert_eq!(libretas[0].url, "https://otra.ejemplo.com/x/");
    }

    /// Una libreta sin nombre igual se muestra: es donde puede estar el
    /// contacto que la persona busca.
    #[test]
    fn una_libreta_sin_nombre_se_muestra_igual() {
        let xml = LIBRETAS.replace("<d:displayname>Personal</d:displayname>", "");
        assert_eq!(libretas_de(&xml, "https://x/").len(), 1);
    }

    const TARJETAS: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
  <d:response>
    <d:href>/dav/addressbooks/users/ana/personal/ana.vcf</d:href>
    <d:propstat><d:prop><c:address-data>BEGIN:VCARD
VERSION:3.0
FN:Ana Pérez
N:Pérez;Ana;;;
EMAIL:ana@ejemplo.com
END:VCARD
</c:address-data></d:prop></d:propstat>
  </d:response>
</d:multistatus>"#;

    /// La dirección va con la tarjeta porque es lo que la identifica en el
    /// servidor: el `UID` de adentro lo escribe quien la creó y puede faltar,
    /// estar repetido, o ser el mismo en dos libretas distintas.
    #[test]
    fn la_tarjeta_sale_con_su_direccion() {
        let tarjetas = tarjetas_de(TARJETAS, "https://nube.ejemplo.com/dav/addressbooks/users/ana/personal/");

        assert_eq!(tarjetas.len(), 1);
        assert!(tarjetas[0].0.ends_with("/ana.vcf"), "{}", tarjetas[0].0);
        assert!(tarjetas[0].1.contains("Ana Pérez"));
    }

    #[test]
    fn un_xml_roto_no_da_nada() {
        assert!(libretas_de("no es xml", "https://x/").is_empty());
        assert!(tarjetas_de("<abierto>", "https://x/").is_empty());
        assert!(libretas_de("", "https://x/").is_empty());
    }

    #[test]
    fn las_consultas_son_xml_valido() {
        assert!(roxmltree::Document::parse(&consulta_de_libretas()).is_ok());
        assert!(roxmltree::Document::parse(&consulta_de_tarjetas()).is_ok());
    }
}
