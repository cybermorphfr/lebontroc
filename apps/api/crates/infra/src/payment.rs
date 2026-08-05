//! Fournisseur de paiement derrière un trait (F4.2) : le domaine ne parle
//! qu'en opérations métier (préautoriser, capturer, libérer) — Mangopay,
//! Stripe Connect ou le simulateur de la bêta sont des détails d'infra.
//!
//! Bêta fermée : `FakePaymentProvider` simule un PSP (aucun argent réel).
//! Le branchement Mangopay sandbox se fera derrière ce même trait, sans
//! toucher ni au domaine ni aux handlers.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

/// Demande de préautorisation d'une soulte.
pub struct PreauthRequest<'a> {
    /// Clé d'idempotence côté PSP, déterministe : `trade-{id}`.
    pub reference: &'a str,
    pub amount_cents: i32,
    /// Numéro de carte (simulation en bêta ; tokenisé chez un vrai PSP).
    pub card_number: &'a str,
}

/// Issue d'une demande de préautorisation.
#[derive(Debug, Clone)]
pub enum PreauthOutcome {
    /// Fonds bloqués chez la banque du payeur : la soulte est séquestrée.
    Escrowed { provider_ref: String },
    /// Refus du PSP ou de la banque — retentable jusqu'à la date limite.
    Failed { reason: String },
    /// Authentification 3DS requise : rediriger le payeur (flux Mangopay,
    /// jamais émis par le simulateur).
    Pending {
        provider_ref: String,
        secure_mode_url: String,
    },
}

#[async_trait]
pub trait PaymentProvider: Send + Sync {
    /// Nom stocké dans `payments.provider` (`fake`, `mangopay`).
    fn name(&self) -> &'static str;
    /// Bloque `amount_cents` sur la carte du payeur.
    async fn preauthorize(&self, request: PreauthRequest<'_>) -> anyhow::Result<PreauthOutcome>;
    /// Capture une préautorisation (remise confirmée). `fees_cents` revient
    /// à la plateforme, le reste au bénéficiaire.
    async fn capture(
        &self,
        provider_ref: &str,
        amount_cents: i32,
        fees_cents: i32,
    ) -> anyhow::Result<()>;
    /// Libère une préautorisation sans capture (troc annulé).
    async fn cancel(&self, provider_ref: &str) -> anyhow::Result<()>;
}

/// Simulateur de PSP pour la bêta fermée et les tests : déterministe via des
/// cartes magiques, idempotent par référence. Les capture/libérations de
/// références inconnues réussissent silencieusement — l'état en mémoire ne
/// survit pas à un redémarrage et la table `payments` reste la source de
/// vérité.
#[derive(Default)]
pub struct FakePaymentProvider {
    preauths: Mutex<HashMap<String, PreauthOutcome>>,
}

/// Carte magique du simulateur : refus banque.
pub const FAKE_CARD_DECLINED_SUFFIX: &str = "0002";
/// Carte magique du simulateur : provision insuffisante.
pub const FAKE_CARD_INSUFFICIENT_SUFFIX: &str = "9995";

impl FakePaymentProvider {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PaymentProvider for FakePaymentProvider {
    fn name(&self) -> &'static str {
        "fake"
    }

    async fn preauthorize(&self, request: PreauthRequest<'_>) -> anyhow::Result<PreauthOutcome> {
        let outcome = if request.card_number.ends_with(FAKE_CARD_DECLINED_SUFFIX) {
            PreauthOutcome::Failed {
                reason: "carte_refusee".to_string(),
            }
        } else if request.card_number.ends_with(FAKE_CARD_INSUFFICIENT_SUFFIX) {
            PreauthOutcome::Failed {
                reason: "provision_insuffisante".to_string(),
            }
        } else {
            PreauthOutcome::Escrowed {
                provider_ref: format!("fake-{}", uuid::Uuid::new_v4()),
            }
        };
        // Idempotence par référence, comme un vrai PSP : rejouer la même
        // demande renvoie le même résultat (sauf après un échec, retentable).
        let mut preauths = self.preauths.lock().expect("verrou simulateur");
        match preauths.get(request.reference) {
            Some(existing @ PreauthOutcome::Escrowed { .. }) => Ok(existing.clone()),
            _ => {
                preauths.insert(request.reference.to_string(), outcome.clone());
                Ok(outcome)
            }
        }
    }

    async fn capture(
        &self,
        _provider_ref: &str,
        _amount_cents: i32,
        _fees_cents: i32,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn cancel(&self, _provider_ref: &str) -> anyhow::Result<()> {
        Ok(())
    }
}
