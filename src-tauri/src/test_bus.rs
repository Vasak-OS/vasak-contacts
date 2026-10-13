//! Un bus de sesión privado para las pruebas que pasan por el cliente de
//! `tauri-plugin-vsk-store`: un `dbus-daemon` por prueba, con un almacén de
//! mentira que toma el nombre del sincronizador.
//!
//! Privado a propósito: la configuración de sesión del sistema trae los
//! archivos de activación, y `StartServiceByName` arrancaría el sincronizador
//! de verdad de quien corre las pruebas.
//!
//! `StoreClient::verified` abre el bus **de sesión**, así que cada prueba que
//! llega hasta ahí apunta `DBUS_SESSION_BUS_ADDRESS` a su bus privado. Es una
//! variable del proceso: estas pruebas van de a una, con [`exclusive`].

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tauri_plugin_vsk_store::client::{OwnerCheck, SERVICE};
use tauri_plugin_vsk_store::StoreClient;
use zbus::Connection;

static LOCK: Mutex<()> = Mutex::new(());
static NEXT: AtomicUsize = AtomicUsize::new(0);

/// Que las pruebas del bus no se pisen la variable de entorno. Una prueba que
/// falló no envenena a las demás.
pub fn exclusive() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Un `dbus-daemon` sólo para una prueba, sin activación de servicios.
pub struct PrivateBus {
    child: Child,
    config: PathBuf,
    pub address: String,
}

impl PrivateBus {
    pub fn start() -> Self {
        let dir = std::env::temp_dir();
        let config = dir.join(format!(
            "vsk-store-app-test-{}-{}.conf",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::write(
            &config,
            format!(
                r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir={}</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#,
                dir.display()
            ),
        )
        .unwrap();
        let mut child = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .arg("--nofork")
            .arg("--print-address")
            .stdout(Stdio::piped())
            .spawn()
            .expect("hace falta `dbus-daemon` para estas pruebas (paquete dbus-daemon)");
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let address = address.trim().to_string();
        assert!(!address.is_empty(), "dbus-daemon no dio su dirección");
        let bus = Self {
            child,
            config,
            address,
        };
        // Que `StoreClient::verified` abra este bus como el de sesión.
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &bus.address);
        bus
    }

    /// Publica `object` en `path` y toma el nombre del sincronizador.
    pub async fn serve<I: zbus::object_server::Interface>(
        &self,
        path: &str,
        object: I,
    ) -> Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .serve_at(path, object)
            .unwrap()
            .name(SERVICE)
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.config);
    }
}

/// La comprobación del dueño, decidida por la prueba: el servicio de mentira
/// corre en este mismo proceso, que no es `/usr/bin/vasak-accounts-sync`.
struct Decided(bool);

impl OwnerCheck for Decided {
    fn trusted(&self, _pid: u32) -> bool {
        self.0
    }
}

/// El cliente del plugin, que le cree al dueño del nombre si `trusted`.
pub fn client(trusted: bool) -> StoreClient {
    StoreClient::with_check(Arc::new(Decided(trusted)))
}

/// Lo que se le pidió a un servicio de mentira, en orden.
#[derive(Clone, Default)]
pub struct Calls(Arc<Mutex<Vec<String>>>);

impl Calls {
    pub fn push(&self, call: String) {
        self.0.lock().unwrap().push(call);
    }

    pub fn taken(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}
