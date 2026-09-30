//! Interpretar una tarjeta de contacto.
//!
//! ── De dónde viene esto ─────────────────────────────────────────────────────
//!
//! De la libreta de la persona, pero **lo escribió cualquiera**: una tarjeta
//! puede haber llegado adjunta a un correo, importada de un teléfono viejo, o
//! sincronizada desde un servidor compartido. No es contenido de confianza sólo
//! por estar en la libreta de alguien.
//!
//! Por eso el parseo tiene topes, no tiene `unsafe`, y todo lo que no se
//! entiende devuelve algo razonable en vez de cortar. Una tarjeta rota no puede
//! impedir ver las otras trescientas.
//!
//! ── Las tres versiones ──────────────────────────────────────────────────────
//!
//! Conviven la 2.1, la 3.0 y la 4.0, y las diferencias que importan son pocas
//! pero muerden:
//!
//! - En **2.1** los parámetros van sueltos (`TEL;HOME;VOICE:`) y el texto puede
//!   venir en `quoted-printable`. La escriben los teléfonos viejos y los
//!   exportadores de agendas de hace veinte años, que es justo lo que la gente
//!   tiene guardado.
//! - En **3.0** los parámetros llevan nombre (`TEL;TYPE=HOME:`) y el juego de
//!   caracteres puede ser cualquiera.
//! - En **4.0** todo es UTF-8 y las direcciones llevan `mailto:`.
//!
//! Se leen las tres. Escribir es otra cosa y todavía no se hace.//!
//! ── Dónde está cada cosa ────────────────────────────────────────────────────
//!
//! - [`lines`]: juntar lo plegado, separar nombre, parámetros y valor, y
//!   deshacer las codificaciones. Lo que es igual para cualquier propiedad.
//! - [`contact`]: el contacto que se muestra y cómo se arma.
//! - [`date`], [`photo`] y [`social`]: las propiedades que tienen trabajo
//!   propio.

pub mod contact;
pub mod date;
pub mod lines;
pub mod photo;
pub mod social;

pub use contact::{contact_from, Contact};
pub use lines::split_cards;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod real_cards;
