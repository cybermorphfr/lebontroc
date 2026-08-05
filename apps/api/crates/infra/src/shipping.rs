//! Fournisseur d'expédition derrière un trait (F4.3) — même pattern
//! mock-first que le paiement. Le contrat mime la structure Boxtal v3
//! (cible du branchement réel) : la bascule sera un adaptateur, pas une
//! réécriture.
//!
//! Bêta fermée : `FakeShippingProvider` — relais simulés déterministes,
//! étiquette réduite à un code de dépôt, et un suivi qui saute directement
//! du dépôt à l'arrivée en relais (le transit réel viendra des webhooks).

use async_trait::async_trait;

/// Un point relais proposé au destinataire.
#[derive(Debug, Clone)]
pub struct Relay {
    pub code: String,
    pub name: String,
    pub address: String,
}

/// Demande d'étiquette point relais.
pub struct LabelRequest<'a> {
    /// Clé d'idempotence : `shipment-{id}`.
    pub reference: &'a str,
    /// Format forfaitaire (`s`, `m`, `l`).
    pub format: &'a str,
    /// Relais de destination choisi par le destinataire.
    pub relay_code: &'a str,
}

/// Une étiquette générée.
#[derive(Debug, Clone)]
pub struct Label {
    pub provider_ref: String,
    /// Code à présenter au comptoir du relais de dépôt.
    pub drop_code: String,
}

/// Statut de suivi normalisé (sous-ensemble des statuts transporteur).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingStatus {
    Pending,
    InTransit,
    ArrivedAtRelay,
    PickedUp,
}

#[async_trait]
pub trait ShippingProvider: Send + Sync {
    /// Nom stocké dans `shipments.provider` (`fake`, `boxtal`).
    fn name(&self) -> &'static str;
    /// Relais proches d'un code postal, pour le choix du destinataire.
    async fn search_relays(&self, postal_code: &str) -> anyhow::Result<Vec<Relay>>;
    async fn create_label(&self, request: LabelRequest<'_>) -> anyhow::Result<Label>;
    /// Suivi (polling de secours ; mécanisme principal du simulateur).
    async fn tracking_status(&self, provider_ref: &str) -> anyhow::Result<TrackingStatus>;
    /// Annule une étiquette non utilisée (troc annulé avant dépôt).
    async fn cancel_label(&self, provider_ref: &str) -> anyhow::Result<()>;
}

/// Simulateur : relais déterministes par code postal, étiquettes toujours
/// accordées, colis « arrivé » dès que le dépôt est déclaré. Tolérant aux
/// références inconnues (l'état ne survit pas à un redémarrage — la table
/// `shipments` est la source de vérité).
#[derive(Default)]
pub struct FakeShippingProvider;

impl FakeShippingProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ShippingProvider for FakeShippingProvider {
    fn name(&self) -> &'static str {
        "fake"
    }

    async fn search_relays(&self, postal_code: &str) -> anyhow::Result<Vec<Relay>> {
        let cp = if postal_code.len() == 5 {
            postal_code
        } else {
            "00000"
        };
        Ok(vec![
            Relay {
                code: format!("FR-{cp}-001"),
                name: "Tabac-presse de la Gare (simulé)".to_string(),
                address: format!("2 place de la Gare, {cp}"),
            },
            Relay {
                code: format!("FR-{cp}-002"),
                name: "Supérette du Port (simulé)".to_string(),
                address: format!("18 quai des Trocs, {cp}"),
            },
            Relay {
                code: format!("FR-{cp}-003"),
                name: "Pressing des Lilas (simulé)".to_string(),
                address: format!("41 rue des Lilas, {cp}"),
            },
        ])
    }

    async fn create_label(&self, request: LabelRequest<'_>) -> anyhow::Result<Label> {
        // Déterministe par référence : rejouer la demande redonne la même
        // étiquette (idempotence façon PSP).
        let seed: u32 = request
            .reference
            .bytes()
            .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32))
            % 100_000_000;
        Ok(Label {
            provider_ref: format!("fake-ship-{}", request.reference),
            drop_code: format!("LBT{seed:08}"),
        })
    }

    async fn tracking_status(&self, _provider_ref: &str) -> anyhow::Result<TrackingStatus> {
        // Le simulateur ne connaît pas le dépôt (déclaré à l'app, pas à
        // lui) : il répond « arrivé » — l'app n'interroge le suivi qu'après
        // un dépôt déclaré, le colis saute donc directement au relais.
        Ok(TrackingStatus::ArrivedAtRelay)
    }

    async fn cancel_label(&self, _provider_ref: &str) -> anyhow::Result<()> {
        Ok(())
    }
}
