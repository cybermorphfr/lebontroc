//! Logique métier pure de Lebontroc — aucun IO, aucune dépendance infra.
//!
//! C'est ici que vivront les règles de troc (plafond de soulte, machine à
//! états du troc, acceptation atomique). Pour F0.1, seul le statut de santé
//! applicatif est modélisé.

pub mod auth;
pub mod catalog;
pub mod dispute;
pub mod health;
pub mod moderation;
pub mod payment;
pub mod review;
pub mod shipping;
pub mod trade;
