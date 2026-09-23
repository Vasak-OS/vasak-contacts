//! Las libretas de las cuentas en línea.
//!
//! ── Qué pide esta aplicación, y qué no ──────────────────────────────────────
//!
//! `account.contacts` y nada más. El correo es de la aplicación de correo, los
//! eventos de la de calendario y los archivos del gestor de archivos.
//!
//! El límite no es sólo una convención: `vasak-permissions` lo tiene declarado
//! para este binario, así que un pedido fuera de ahí se niega sin siquiera
//! preguntarle a la persona. Y acá importa especialmente: los contactos y los
//! calendarios viven en el **mismo servidor** y detrás de la **misma
//! contraseña** —un Nextcloud entrega los dos con la misma credencial de
//! aplicación—, así que sin esa línea la separación entre esta aplicación y la
//! de calendario sería un `const` que se puede cambiar.
//!
//! ── Dónde vive la contraseña ────────────────────────────────────────────────
//!
//! No acá. La cuenta se conecta una vez desde Configuración y esta aplicación le
//! pide al servicio la credencial cuando la necesita — la primera vez con un
//! diálogo de permiso, como cualquier otra aplicación. Nadie escribe una
//! contraseña en esta ventana.

use serde::{Deserialize, Serialize};

const SERVICE: &str = "ar.net.vasak.os.AccountManager";
const PATH: &str = "/ar/net/vasak/os/AccountManager";
const INTERFACE: &str = "ar.net.vasak.os.AccountManager";

/// La capacidad que esta aplicación puede pedir. La única.
const CAPACIDAD: &str = "contacts";

#[derive(Debug, Clone, Deserialize)]
struct Resumen {
    id: String,
    display_name: String,
    capabilities: Vec<String>,
    #[serde(default)]
    needs_reauth: bool,
}

/// Una cuenta con libretas, para mostrar de dónde salen los contactos.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CuentaConLibreta {
    pub id: String,
    pub nombre: String,
    /// Si hay que reconectarla desde Configuración. Se muestra igual: una cuenta
    /// que desaparece de la lista parece una cuenta que se borró.
    pub necesita_reconectarse: bool,
}

/// Lo necesario para hablar con el servidor de una cuenta.
///
/// **Sin `Debug` derivado**: `secreto` es la contraseña o el token de la cuenta,
/// y un `Debug` derivado lo escribiría entero en cualquier registro, en
/// cualquier `dbg!` de paso y en el mensaje de cualquier pánico que lo lleve
/// adentro.
#[derive(Clone, PartialEq, Eq)]
pub struct Credencial {
    /// La dirección donde vive la libreta de la persona.
    pub home: String,
    pub usuario: String,
    pub secreto: String,
    /// Qué **es** ese secreto, que decide cómo se manda.
    pub auth: AuthKind,
}

/// Cómo autenticarse contra el servidor.
///
/// Se decide por **lo que guardó el servicio de cuentas** y no por el proveedor:
/// una cuenta de Google conectada por OAuth2 y una conectada como servidor
/// personalizado con contraseña se ven igual desde acá. La marca es el
/// `client_id`, que sólo tienen las que pasaron por un flujo OAuth2 —lo escribe
/// el servicio junto con las URLs para renovar el token—. Es el mismo criterio
/// que usa el sincronizador de correo, y por el mismo motivo.
///
/// Confundirlas manda una contraseña donde va un token: el servidor contesta un
/// rechazo que parece de credenciales y manda a revisar la contraseña de una
/// cuenta que está perfecta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    /// Usuario y contraseña, en una cabecera `Basic`.
    Password,
    /// Un token de acceso, en una cabecera `Bearer`. Google no acepta otra cosa.
    Token,
}

impl std::fmt::Debug for Credencial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credencial")
            .field("home", &self.home)
            .field("usuario", &self.usuario)
            .field("secreto", &"<tachado>")
            .field("auth", &self.auth)
            .finish()
    }
}

async fn conectar() -> Result<zbus::Connection, String> {
    zbus::Connection::system().await.map_err(|e| {
        format!(
            "no se pudo contactar al gestor de cuentas: {e}. \
             Comprobá que vasak-accounts esté en ejecución."
        )
    })
}

async fn llamar<A>(
    conexion: &zbus::Connection,
    metodo: &str,
    argumentos: &A,
) -> Result<String, String>
where
    A: serde::ser::Serialize + zbus::zvariant::DynamicType,
{
    conexion
        .call_method(Some(SERVICE), PATH, Some(INTERFACE), metodo, argumentos)
        .await
        .map_err(|e| format!("{metodo}: {e}"))?
        .body()
        .deserialize()
        .map_err(|e| format!("respuesta inválida de {metodo}: {e}"))
}

/// Las cuentas que tienen libretas.
///
/// **No pide permiso**: listar es metadatos, y el servicio ya acota lo que
/// devuelve al usuario que pregunta. El permiso se pide al leer los contactos,
/// que es cuando hace falta la credencial.
pub async fn cuentas() -> Result<Vec<CuentaConLibreta>, String> {
    let conexion = conectar().await?;
    let json = llamar(&conexion, "ListAccounts", &()).await?;

    let resumenes: Vec<Resumen> =
        serde_json::from_str(&json).map_err(|e| format!("no se pudo leer la lista: {e}"))?;

    Ok(resumenes
        .into_iter()
        .filter(|r| r.capabilities.iter().any(|c| c == CAPACIDAD))
        .map(|r| CuentaConLibreta {
            id: r.id,
            nombre: r.display_name,
            necesita_reconectarse: r.needs_reauth,
        })
        .collect())
}

/// Le pide al servicio con qué hablarle al servidor de una cuenta.
///
/// **Acá sí se pregunta**, y la primera vez la persona ve el diálogo de permiso.
pub async fn credencial_de(account_id: &str) -> Result<Credencial, String> {
    let conexion = conectar().await?;

    // El token primero: es lo que dispara el diálogo, y si la persona dice que
    // no, no tiene sentido haber pedido el resto.
    let secreto = llamar(&conexion, "GetAccessToken", &(account_id, CAPACIDAD)).await?;
    let datos = llamar(&conexion, "GetAccountData", &(account_id, CAPACIDAD)).await?;

    let datos: serde_json::Value =
        serde_json::from_str(&datos).map_err(|e| format!("no se pudo leer la cuenta: {e}"))?;
    // El servicio devuelve la cuenta entera con la capacidad adentro.
    let config = datos.get("config").unwrap_or(&datos);

    credencial_desde(config, secreto)
}

/// Arma la credencial a partir de lo que guardó el servicio al conectar.
pub fn credencial_desde(config: &serde_json::Value, secreto: String) -> Result<Credencial, String> {
    let campo = |nombre: &str| config.get(nombre).and_then(|v| v.as_str());

    let home = campo("url").ok_or(
        "la cuenta no guardó la dirección de su libreta; \
         volvé a conectarla desde Configuración",
    )?;
    let usuario = campo("username")
        .ok_or("la cuenta no guardó el usuario")?
        .to_string();

    if !home.starts_with("https://") {
        return Err(format!(
            "«{home}» no está cifrado, y por ahí la contraseña de la cuenta \
             viajaría a la vista de cualquiera en la red"
        ));
    }

    // El `client_id` es la marca de que la cuenta pasó por un flujo OAuth2, así
    // que el secreto es un token y no una contraseña. Ver `AuthKind`.
    let auth = if campo("client_id").is_some() {
        AuthKind::Token
    } else {
        AuthKind::Password
    };

    Ok(Credencial {
        home: home.to_string(),
        usuario,
        secreto,
        auth,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// **El límite de esta aplicación.** Pide `contacts` y nada más.
    ///
    /// Importa especialmente acá: los contactos y los calendarios viven en el
    /// mismo servidor y detrás de la misma contraseña —un Nextcloud entrega los
    /// dos con la misma credencial de aplicación—, así que sin este límite la
    /// separación entre esta aplicación y la de calendario sería un `const` que
    /// se puede cambiar.
    ///
    /// `vasak-permissions` lo tiene declarado para este binario, así que un
    /// pedido fuera de ahí se niega sin preguntar. Este test está para que
    /// agregar otra capacidad acá sea una decisión y no un descuido.
    #[test]
    fn esta_aplicacion_solo_pide_los_contactos() {
        assert_eq!(CAPACIDAD, "contacts");
    }

    /// El secreto no puede aparecer en un registro ni en un pánico.
    #[test]
    fn el_secreto_no_se_imprime() {
        let credencial = Credencial {
            home: "https://nube.ejemplo.com/dav/addressbooks/users/ana/".into(),
            usuario: "ana".into(),
            secreto: "la-contrasena-de-verdad".into(),
            auth: AuthKind::Password,
        };

        let impreso = format!("{credencial:?}");
        assert!(!impreso.contains("la-contrasena-de-verdad"), "{impreso}");
        // Y lo que sirve para diagnosticar se sigue viendo.
        assert!(impreso.contains("nube.ejemplo.com"), "{impreso}");
        assert!(impreso.contains("ana"), "{impreso}");
    }

    #[test]
    fn la_credencial_sale_de_lo_que_guardo_el_servicio() {
        let config = json!({
            "url": "https://nube.ejemplo.com/remote.php/dav/addressbooks/users/ana/",
            "username": "ana",
            "auth": "basic",
        });

        let credencial = credencial_desde(&config, "la-contrasena".into()).unwrap();
        assert_eq!(credencial.usuario, "ana");
        assert!(credencial.home.ends_with("/users/ana/"));
    }

    /// Una cuenta con contraseña se autentica con contraseña.
    ///
    /// Es la de Nextcloud y la del servidor escrito a mano: lo que guardó el
    /// servicio no tiene `client_id` porque nunca pasó por un flujo OAuth2.
    #[test]
    fn sin_client_id_el_secreto_es_una_contrasena() {
        let config = json!({
            "url": "https://nube.ejemplo.com/remote.php/dav/addressbooks/users/ana/",
            "username": "ana",
        });

        let credencial = credencial_desde(&config, "la-contrasena".into()).unwrap();
        assert_eq!(credencial.auth, AuthKind::Password);
    }

    /// Y una de Google, con token.
    ///
    /// La marca es el `client_id`, que sólo lo escribe el servicio cuando la
    /// cuenta pasó por OAuth2. Mandarle `Basic` a Google da 401, y el rechazo
    /// parece de credenciales: manda a revisar la contraseña de una cuenta que
    /// está perfecta.
    #[test]
    fn con_client_id_el_secreto_es_un_token() {
        let config = json!({
            "url": "https://www.googleapis.com/carddav/v1/principals/ana@gmail.com/",
            "username": "ana@gmail.com",
            "client_id": "algo.apps.googleusercontent.com",
            "token_url": "https://oauth2.googleapis.com/token",
        });

        let credencial = credencial_desde(&config, "el-token".into()).unwrap();
        assert_eq!(credencial.auth, AuthKind::Token);
        assert_eq!(credencial.usuario, "ana@gmail.com");
    }

    /// Sin cifrar no se habla: por ahí la contraseña de la cuenta viajaría en
    /// claro. El servicio ya exige HTTPS al conectar, así que llegar acá con
    /// `http://` es una cuenta armada a mano contra esa recomendación.
    #[test]
    fn una_direccion_sin_cifrar_se_rechaza() {
        let config = json!({ "url": "http://nube.ejemplo.com/dav/", "username": "ana" });
        let error = credencial_desde(&config, "x".into()).unwrap_err();
        assert!(error.contains("cifrado"), "{error}");
    }

    /// El mensaje tiene que decir qué hacer. Una cuenta a la que le falta la
    /// dirección se conectó antes de que se guardara, y lo que corresponde es
    /// reconectarla.
    #[test]
    fn una_cuenta_sin_direccion_dice_que_se_reconecte() {
        let sin_url = json!({ "username": "ana" });
        let error = credencial_desde(&sin_url, "x".into()).unwrap_err();
        assert!(error.contains("volvé a conectarla"), "{error}");
    }

    /// Sólo las cuentas con libreta. Una de sólo correo no tiene nada que
    /// mostrar acá.
    #[test]
    fn solo_se_listan_las_cuentas_con_libreta() {
        let json = r#"[
            {"id":"a","display_name":"Nube","capabilities":["contacts","drive"],"needs_reauth":false},
            {"id":"b","display_name":"Correo","capabilities":["email"],"needs_reauth":false}
        ]"#;
        let resumenes: Vec<Resumen> = serde_json::from_str(json).unwrap();

        let con_libreta: Vec<&Resumen> = resumenes
            .iter()
            .filter(|r| r.capabilities.iter().any(|c| c == CAPACIDAD))
            .collect();

        assert_eq!(con_libreta.len(), 1);
        assert_eq!(con_libreta[0].id, "a");
    }
}
