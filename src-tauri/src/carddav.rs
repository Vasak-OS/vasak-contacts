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
//!
//! ── Qué se le cree al servidor ──────────────────────────────────────────────
//!
//! Lo que contesta lo escribió cualquiera, y esta aplicación lleva la credencial
//! de la cuenta. Por eso **cada dirección que el servidor manda se resuelve
//! contra la de la cuenta y se rechaza si es de otro origen**, y el cliente
//! habla **sólo `https`** y **sin redirecciones**. Y lo que llega pasa por los
//! topes de `dav::Limits` antes de armarse: `roxmltree` baja de forma recursiva
//! —una respuesta con doscientos mil niveles de anidado **aborta la
//! aplicación**—, y con miles de espacios de nombres o de atributos se traba.
//! Todo eso, y el rechazo de otras direcciones, está en [`crate::dav`].

use std::time::Duration;

use base64::Engine;
use reqwest::Url;
use serde::Serialize;

use crate::cuentas::{AuthKind, Credencial};
use crate::dav::{self, DavError, Limits};
use crate::vcard::{self, Contacto};

const TIMEOUT: Duration = Duration::from_secs(30);

/// Los topes de esta aplicación, con los mismos números que usa el sincronizador
/// de `vasak-accounts` para las mismas respuestas de DAV.
const LIMITES: Limits = Limits::DEFAULT;

/// Tope de lo que se lee de una respuesta.
///
/// Una agenda de mil contactos son unos pocos megabytes. Dieciséis es de sobra
/// y corta un servidor que devuelve basura antes de que la memoria de la
/// ventana crezca sin freno. Es [`Limits::max_body_bytes`], y vive en `dav`
/// porque el tope no es de este protocolo sino de HTTP.
const MAX_CUERPO: usize = Limits::DEFAULT.max_body_bytes;

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
///
/// **Un XML que no se entiende no es una libreta vacía.** Devolver una lista
/// vacía hacía que una respuesta cortada a la mitad —una conexión que se
/// interrumpió, un servidor que contestó una página de error— se viera igual
/// que «esta cuenta no tiene libretas». La persona miraría la pantalla vacía
/// creyendo que perdió sus contactos.
pub fn libretas_de(xml: &str, base: &Url) -> Result<Vec<Libreta>, DavError> {
    let documento = dav::parse_xml(xml, &LIMITES)?;
    let base = base.clone();

    Ok(documento
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
            // La dirección se **resuelve y se compara**, no se pega: un `href`
            // de otro servidor —o con usuario y contraseña adentro, o de más de
            // 2 KiB— no sale. Antes se resolvía con `Url::join` y nada más, y esa
            // dirección era la que después llevaba la cabecera de autenticación.
            let Ok(url) = dav::resolve_href(&base, href) else {
                return None;
            };
            let url = url.to_string();

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
                nombre: if nombre.is_empty() {
                    "Contactos".into()
                } else {
                    nombre
                },
            })
        })
        .collect())
}

/// Saca las tarjetas y su dirección de una respuesta `REPORT`.
///
/// La dirección va con la tarjeta porque es lo que la identifica en el
/// servidor: el `UID` de adentro lo escribe quien la creó y puede faltar, estar
/// repetido, o ser el mismo en dos libretas distintas.
pub fn tarjetas_de(xml: &str, base: &Url) -> Result<Vec<(String, String)>, DavError> {
    let documento = dav::parse_xml(xml, &LIMITES)?;
    let base = base.clone();

    Ok(documento
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
            // Un `href` de otro origen **descarta la tarjeta**: se pediría con
            // la credencial de la cuenta puesta. Una sin `href` es lo mismo —no
            // hay contra qué compararla—, y antes se guardaba con la dirección
            // vacía, que después no reconocía ninguna.
            let Ok(url) = dav::resolve_href(&base, href) else {
                return None;
            };

            Some((url.to_string(), datos.to_string()))
        })
        .collect())
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

/// La cabecera `Authorization` que le corresponde a esta cuenta.
///
/// `Basic` para una contraseña y `Bearer` para un token. No es una preferencia:
/// Google contesta 401 a cualquier `Basic`, y un servidor que espera contraseña
/// no entiende un `Bearer`. Cuál va lo decide lo que guardó el servicio de
/// cuentas, no el proveedor — ver `cuentas::AuthKind`.
fn cabecera_de(credencial: &Credencial) -> String {
    match credencial.auth {
        AuthKind::Password => format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD
                .encode(format!("{}:{}", credencial.usuario, credencial.secreto))
        ),
        AuthKind::Token => format!("Bearer {}", credencial.secreto),
    }
}

/// El cliente HTTP de esta aplicación: sólo `https` y sin redirecciones.
///
/// Va por [`dav::client`] y no en línea, para que las dos cosas que no se
/// negocian —que la credencial no viaje en claro y que no se siga a otro
/// servidor— no puedan quedar afuera si mañana se agrega un cliente nuevo.
/// Antes eran sólo la segunda: `redirect(Policy::none())` estaba, y `https_only`
/// no.
fn cliente() -> Result<reqwest::Client, String> {
    dav::client(TIMEOUT)
}

/// La respuesta del servidor, con tope y con los estados raros como error.
///
/// Los textos son de [`DavError`], y son **fijos**: sin la dirección del servidor
/// y sin lo que escribió el otro lado. La ventana los muestra a la persona y
/// cualquier programa de la sesión los puede leer del estado, así que no pueden
/// llevar el nombre de la máquina ni los del certificado que dio el otro lado.
async fn cuerpo_con_tope(mut respuesta: reqwest::Response) -> Result<String, String> {
    let estado = respuesta.status();
    if estado == reqwest::StatusCode::UNAUTHORIZED {
        return Err(DavError::Unauthorized.to_string());
    }
    // Una redirección no se sigue —`dav::client` lo tiene así— y acá es un error
    // y no un estado más: el `3xx` no trae las tarjetas, y seguirlo mandaría la
    // credencial a donde el servidor dijera.
    if estado.is_redirection() {
        return Err(DavError::Redirect(estado.as_u16()).to_string());
    }
    if !estado.is_success() {
        return Err(DavError::Status(estado.as_u16()).to_string());
    }

    // Por trozos y cortando en el momento: leer todo y medir después es
    // enterarse del problema cuando ya pasó — un servidor que manda gigabytes
    // hace crecer la memoria de la ventana hasta donde quiera.
    let mut cuerpo = Vec::new();
    while let Some(trozo) = respuesta
        .chunk()
        .await
        .map_err(|e| DavError::network(e.without_url()).to_string())?
    {
        if cuerpo.len() + trozo.len() > MAX_CUERPO {
            return Err(DavError::BodyTooLarge(MAX_CUERPO).to_string());
        }
        cuerpo.extend_from_slice(&trozo);
    }

    Ok(String::from_utf8_lossy(&cuerpo).into_owned())
}

/// La dirección de la cuenta, ya comprobada: `https` y sin usuario y contraseña
/// adentro.
fn direccion_de(credencial: &crate::cuentas::Credencial) -> Result<Url, String> {
    dav::parse_account_url(&credencial.home).map_err(|e| e.to_string())
}

/// Las libretas que hay en la carpeta de la persona.
pub async fn libretas(credencial: &crate::cuentas::Credencial) -> Result<Vec<Libreta>, String> {
    let home = direccion_de(credencial)?;

    let respuesta = cliente()?
        .request(metodo("PROPFIND"), home.clone())
        .header("Authorization", cabecera_de(credencial))
        // 1: la carpeta y lo que hay dentro. Con 0 sólo vendría la carpeta, que
        // es justo lo que no interesa.
        .header("Depth", "1")
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(consulta_de_libretas())
        .send()
        .await
        .map_err(|e| DavError::network(e.without_url()).to_string())?;

    let xml = cuerpo_con_tope(respuesta).await?;
    // El XML se arma fuera del hilo del bucle de eventos: una `addressbook-query`
    // de dieciséis megas tarda lo suyo, y mientras lo ocupe la ventana no atiende
    // nada más. Y si se cae por dentro, vuelve como un documento que no se
    // entiende en vez de llevarse la tarea.
    dav::off_runtime(move || libretas_de(&xml, &home))
        .await
        .map_err(|e| e.to_string())
}

/// Todos los contactos de una libreta.
///
/// **La dirección se compara con la de la cuenta antes de mandar nada.** Viene
/// de la ventana —o sea, de un proceso de la sesión—, y el `Authorization` va en
/// el mismo pedido. `libretas_de` ya descarta las que no son del mismo origen;
/// esto es para el caso de que se pida una que no vino de la lista.
pub async fn contactos(
    credencial: &crate::cuentas::Credencial,
    libreta: &str,
) -> Result<Vec<Contacto>, String> {
    let home = direccion_de(credencial)?;
    let pedido = dav::resolve_href(&home, libreta).map_err(|e| e.to_string())?;
    if pedido.origin() != home.origin() {
        return Err(DavError::ForeignOrigin.to_string());
    }

    // El `href` de cada tarjeta se resuelve contra **la colección que se
    // pidió**, no contra la de la cuenta. Los servidores contestan con una ruta
    // absoluta casi siempre, pero un `href` relativo —`ana.vcf`, que aparece de
    // vez en cuando— resuelve contra la base: contra la de la cuenta daba
    // `https://host/dav/ana.vcf` en vez de
    // `https://host/dav/addressbooks/.../personal/ana.vcf`, y `Contacto.url`
    // existe justamente para volver a buscar la tarjeta, así que después no la
    // encontraba.
    //
    // El control de origen no cambia: `pedido` ya pasó por `resolve_href` contra
    // `home`, así que su origen es el de la cuenta.
    let respuesta = cliente()?
        .request(metodo("REPORT"), pedido.clone())
        .header("Authorization", cabecera_de(credencial))
        // 1: las tarjetas de esta libreta. El estándar lo pide, y hay
        // servidores que sin esto devuelven vacío.
        .header("Depth", "1")
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(consulta_de_tarjetas())
        .send()
        .await
        .map_err(|e| DavError::network(e.without_url()).to_string())?;

    let xml = cuerpo_con_tope(respuesta).await?;
    let tarjetas = dav::off_runtime(move || tarjetas_de(&xml, &pedido))
        .await
        .map_err(|e| e.to_string())?;

    Ok(tarjetas
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

    fn credencial(auth: AuthKind) -> Credencial {
        Credencial {
            home: "https://servidor.ejemplo.com/dav/".into(),
            usuario: "ana@ejemplo.com".into(),
            secreto: "el-secreto".into(),
            auth,
        }
    }

    /// Una contraseña va en `Basic`, con el usuario delante.
    #[test]
    fn la_contrasena_viaja_en_basic() {
        let cabecera = cabecera_de(&credencial(AuthKind::Password));

        assert!(cabecera.starts_with("Basic "), "{cabecera}");
        let codificado =
            base64::engine::general_purpose::STANDARD.encode("ana@ejemplo.com:el-secreto");
        assert_eq!(cabecera, format!("Basic {codificado}"));
    }

    /// Un token va en `Bearer` y **sin el usuario**: Google contesta 401 a
    /// cualquier `Basic`, y el rechazo parece de credenciales.
    #[test]
    fn el_token_viaja_en_bearer() {
        let cabecera = cabecera_de(&credencial(AuthKind::Token));

        assert_eq!(cabecera, "Bearer el-secreto");
    }

    /// El secreto nunca se codifica en base64 cuando es un token: eso es lo que
    /// hacía que Google lo rechazara, y el modo de fallo es silencioso porque
    /// una cabecera mal armada se ve igual que una bien armada.
    #[test]
    fn las_dos_formas_no_se_parecen() {
        let con_clave = cabecera_de(&credencial(AuthKind::Password));
        let con_token = cabecera_de(&credencial(AuthKind::Token));

        assert_ne!(con_clave, con_token);
    }

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

    fn base() -> Url {
        Url::parse("https://nube.ejemplo.com/dav/addressbooks/users/ana/").unwrap()
    }

    /// La carpeta trae también cosas que no son libretas. Listarlas daría
    /// entradas que al abrirlas no tienen nada.
    #[test]
    fn solo_se_listan_las_colecciones_que_son_libretas() {
        let libretas = libretas_de(LIBRETAS, &base()).unwrap();

        assert_eq!(libretas.len(), 1);
        assert_eq!(libretas[0].nombre, "Personal");
        assert_eq!(
            libretas[0].url,
            "https://nube.ejemplo.com/dav/addressbooks/users/ana/personal/"
        );
    }

    /// Los servidores contestan con una ruta absoluta casi siempre y con una
    /// URL entera a veces, y del mismo origen: pegarlas a mano rompería la
    /// segunda.
    #[test]
    fn un_href_con_url_entera_del_mismo_origen_no_se_pega_dos_veces() {
        let xml = LIBRETAS.replace(
            "<d:href>/dav/addressbooks/users/ana/personal/</d:href>",
            "<d:href>https://nube.ejemplo.com/dav/addressbooks/users/ana/personal/</d:href>",
        );
        let libretas = libretas_de(&xml, &base()).unwrap();
        assert_eq!(libretas.len(), 1);
        assert_eq!(
            libretas[0].url,
            "https://nube.ejemplo.com/dav/addressbooks/users/ana/personal/"
        );
    }

    /// **Una libreta de otro origen no se lista.** Es la fuga de credencial de
    /// `vasak-contacts#39`: el `href` del `multistatus` se resolvía con
    /// `Url::join` y nada más, y esa dirección era la que después llevaba la
    /// cabecera `Authorization` puesta.
    #[test]
    fn una_libreta_de_otro_servidor_no_se_lista() {
        for ajeno in [
            "https://atacante.ejemplo.com/robo/",
            "http://nube.ejemplo.com/dav/addressbooks/users/ana/personal/",
            "https://nube.ejemplo.com:8443/dav/addressbooks/users/ana/personal/",
            "https://ana:secreto@nube.ejemplo.com/dav/addressbooks/users/ana/personal/",
        ] {
            let xml = LIBRETAS.replace(
                "<d:href>/dav/addressbooks/users/ana/personal/</d:href>",
                &format!("<d:href>{ajeno}</d:href>"),
            );
            let libretas = libretas_de(&xml, &base()).unwrap();
            assert!(
                libretas.is_empty(),
                "{ajeno} se listó: {:?}",
                libretas.iter().map(|l| &l.url).collect::<Vec<_>>(),
            );
        }
    }

    /// **Un `href` relativo se resuelve contra la libreta, no contra la cuenta.**
    ///
    /// Es el arreglo del hallazgo de revisión: `contactos()` pedía la `REPORT` a
    /// la libreta pero resolvía los `href` contra `home`, así que un `ana.vcf`
    /// relativo —que los servidores mandan de vez en cuando— quedaba en
    /// `https://host/dav/ana.vcf`. Y `Contacto.url` existe para volver a buscar
    /// la tarjeta, así que después no la encontraba.
    ///
    /// Todos los fixtures de acá usan un `href` **absoluto**, y contra cualquier
    /// base del mismo origen dan lo mismo: por eso el test anterior no lo veía, y
    /// por eso éste usa un relativo, que es el único caso donde la base importa.
    #[test]
    fn un_href_relativo_se_resuelve_contra_la_libreta() {
        let tarjetas = tarjetas_de(
            TARJETA_CON_HREF_RELATIVO,
            &Url::parse("https://nube.ejemplo.com/dav/addressbooks/users/ana/personal/").unwrap(),
        )
        .unwrap();

        assert_eq!(tarjetas.len(), 1);
        assert_eq!(
            tarjetas[0].0, "https://nube.ejemplo.com/dav/addressbooks/users/ana/personal/ana.vcf",
            "el href relativo se resolvió contra otra base",
        );
    }

    /// **El camino de `contactos` completo, menos la red.**
    ///
    /// La prueba anterior llama a `tarjetas_de` con la base escrita a mano, así
    /// que pasa siempre, se haya arreglado el bug o no: el error estaba en **qué
    /// base le pasa `contactos`**, no en `tarjetas_de`. Esta replica las dos
    /// líneas de `contactos` —resolver la libreta contra `home` y usar **esa**
    /// `Url` como base— a propósito, y por eso falla si la regla vuelve a ser la
    /// otra. Contra la base de la cuenta el mismo `href` da otra dirección, que
    /// es justo lo que había que arreglar.
    ///
    /// Lo que queda sin cubrir, y conviene decirlo: que `contactos()` pase esa
    /// `Url` y no otra. Eso no se puede probar sin levantar un servidor, y esta
    /// aplicación no tiene la excepción de `127.0.0.1` en claro que sí tiene
    /// `vasak-accounts` en sus pruebas. Queda fijado por lectura, y es la razón
    /// de que el arreglo sea una línea.
    #[test]
    fn el_camino_de_contactos_resuelve_el_relativo_contra_la_libreta() {
        let home = Url::parse("https://nube.ejemplo.com/dav/").unwrap();
        // Lo que hace `contactos`: resolver la libreta que le pidieron.
        let pedido = dav::resolve_href(
            &home,
            "https://nube.ejemplo.com/dav/addressbooks/users/ana/personal/",
        )
        .unwrap();
        assert_eq!(pedido.origin(), home.origin());

        // Y usarla como base de la respuesta.
        let tarjetas = tarjetas_de(TARJETA_CON_HREF_RELATIVO, &pedido).unwrap();
        assert_eq!(
            tarjetas[0].0,
            "https://nube.ejemplo.com/dav/addressbooks/users/ana/personal/ana.vcf",
        );

        // Contra la de la cuenta daba otra cosa.
        let con_base_de_cuenta = tarjetas_de(TARJETA_CON_HREF_RELATIVO, &home).unwrap();
        assert_eq!(
            con_base_de_cuenta[0].0,
            "https://nube.ejemplo.com/dav/ana.vcf"
        );
    }

    /// Y lo mismo con las tarjetas: una de otro origen se descartaría, porque
    /// `contactos()` la pediría con la credencial de la cuenta. Antes además se
    /// guardaba con la dirección **vacía** si el `href` no se podía resolver,
    /// y eso tampoco servía para nada.
    #[test]
    fn una_tarjeta_de_otro_servidor_no_se_guarda() {
        let xml = TARJETAS.replace(
            "<d:href>/dav/addressbooks/users/ana/personal/ana.vcf</d:href>",
            "<d:href>https://atacante.ejemplo.com/robo/ana.vcf</d:href>",
        );
        assert!(tarjetas_de(&xml, &base()).unwrap().is_empty());
    }

    /// Una libreta sin nombre igual se muestra: es donde puede estar el
    /// contacto que la persona busca.
    #[test]
    fn una_libreta_sin_nombre_se_muestra_igual() {
        let xml = LIBRETAS.replace("<d:displayname>Personal</d:displayname>", "");
        assert_eq!(libretas_de(&xml, &base()).unwrap().len(), 1);
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

    /// La misma respuesta con el `href` **relativo**, que es la forma que los
    /// servidores mandan de vez en cuando y la única donde la base importa.
    const TARJETA_CON_HREF_RELATIVO: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
  <d:response>
    <d:href>ana.vcf</d:href>
    <d:propstat><d:prop><c:address-data>BEGIN:VCARD
VERSION:3.0
FN:Ana Pérez
END:VCARD
</c:address-data></d:prop></d:propstat>
  </d:response>
</d:multistatus>"#;

    /// La dirección va con la tarjeta porque es lo que la identifica en el
    /// servidor: el `UID` de adentro lo escribe quien la creó y puede faltar,
    /// estar repetido, o ser el mismo en dos libretas distintas.
    #[test]
    fn la_tarjeta_sale_con_su_direccion() {
        let tarjetas = tarjetas_de(
            TARJETAS,
            &Url::parse("https://nube.ejemplo.com/dav/addressbooks/users/ana/personal/").unwrap(),
        )
        .unwrap();

        assert_eq!(tarjetas.len(), 1);
        assert!(tarjetas[0].0.ends_with("/ana.vcf"), "{}", tarjetas[0].0);
        assert!(tarjetas[0].1.contains("Ana Pérez"));
    }

    /// **Un XML roto no es una libreta vacía.** Devolver una lista vacía hacía
    /// que una respuesta cortada a la mitad se viera igual que «esta cuenta no
    /// tiene a nadie», y la persona miraría la pantalla creyendo que perdió sus
    /// contactos.
    #[test]
    fn un_xml_roto_se_dice_en_vez_de_parecer_vacio() {
        for basura in ["no es xml", "<abierto>", "", "<a><b></a>"] {
            assert!(
                matches!(libretas_de(basura, &base()), Err(DavError::BadXml(_))),
                "{basura:?}",
            );
            assert!(
                matches!(tarjetas_de(basura, &base()), Err(DavError::BadXml(_))),
                "{basura:?}",
            );
        }
    }

    /// Y un XML válido sin nada adentro **sí** es una libreta vacía: la
    /// diferencia es justamente la que se quería poder decir.
    #[test]
    fn un_xml_valido_y_vacio_si_es_una_libreta_vacia() {
        let vacio = r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:"/>"#;
        assert!(libretas_de(vacio, &base()).unwrap().is_empty());
        assert!(tarjetas_de(vacio, &base()).unwrap().is_empty());
    }

    #[test]
    fn las_consultas_son_xml_valido() {
        assert!(roxmltree::Document::parse(&consulta_de_libretas()).is_ok());
        assert!(roxmltree::Document::parse(&consulta_de_tarjetas()).is_ok());
    }
}
