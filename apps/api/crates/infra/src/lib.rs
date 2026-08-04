//! Accès aux systèmes externes : PostgreSQL (SQLx), e-mail (SMTP), et plus
//! tard S3, Mangopay, logistique.

pub mod analytics;
pub mod auth_repo;
pub mod catalog_repo;
pub mod db;
pub mod email;
pub mod favorites_repo;
pub mod message_repo;
pub mod s3;
pub mod search;
pub mod trade_repo;
