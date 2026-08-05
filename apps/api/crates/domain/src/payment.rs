//! Règles pures du paiement de soulte (F4.2) — sans IO. Le séquestre se fait
//! par préautorisation carte : fonds bloqués à l'acceptation, capturés à la
//! remise, simplement libérés si le troc échoue. La préautorisation expire à
//! 30 jours chez le PSP : l'annulation automatique J+14 des trocs garantit
//! qu'on capture toujours bien avant.

/// Qui doit la soulte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Payeur {
    Proposant,
    Destinataire,
}

/// Le payeur d'une soulte, d'après la direction portée par la proposition.
pub fn payeur(cash_direction: &str) -> Option<Payeur> {
    match cash_direction {
        "du_proposant" => Some(Payeur::Proposant),
        "du_destinataire" => Some(Payeur::Destinataire),
        _ => None,
    }
}

/// Le payeur est devant son écran quand il accepte : il règle tout de suite.
pub const DELAI_PAIEMENT_ACCEPTEUR_MINUTES: i64 = 30;
/// Le payeur n'est pas celui qui accepte : on lui laisse une journée.
pub const DELAI_PAIEMENT_AUTRE_MINUTES: i64 = 24 * 60;

/// Délai accordé au payeur pour préautoriser la soulte, en minutes.
pub fn delai_paiement_minutes(payeur_est_accepteur: bool) -> i64 {
    if payeur_est_accepteur {
        DELAI_PAIEMENT_ACCEPTEUR_MINUTES
    } else {
        DELAI_PAIEMENT_AUTRE_MINUTES
    }
}

/// Commission de la plateforme, en basis points (100 bps = 1 %). Prélevée sur
/// le montant capturé, jamais ajoutée au payeur. 0 pendant la bêta — le champ
/// existe dès le premier jour pour ne pas rétrofitter la machine à états.
pub fn commission_cents(amount_cents: i32, fees_bps: u16) -> i32 {
    ((i64::from(amount_cents) * i64::from(fees_bps)) / 10_000) as i32
}

/// Ce que le bénéficiaire recevra une fois la commission prélevée.
pub fn net_beneficiaire_cents(amount_cents: i32, fees_cents: i32) -> i32 {
    (amount_cents - fees_cents).max(0)
}

/// Statuts de paiement : `en_attente` → `sequestre` | `echoue` | `expire`,
/// puis `sequestre` → `capture` (remise confirmée) | `annule` (troc annulé).
/// Un paiement `echoue` reste retentable jusqu'à la date limite.
pub const STATUTS_PAIEMENT: [&str; 6] = [
    "en_attente",
    "echoue",
    "sequestre",
    "capture",
    "annule",
    "expire",
];

/// La préautorisation peut être (re)tentée.
pub fn peut_tenter_preautorisation(statut: &str) -> bool {
    matches!(statut, "en_attente" | "echoue")
}

/// Le séquestre peut être enregistré.
pub fn peut_sequestrer(statut: &str) -> bool {
    peut_tenter_preautorisation(statut)
}

/// La capture n'a de sens que sur un paiement séquestré.
pub fn peut_capturer(statut: &str) -> bool {
    statut == "sequestre"
}

/// La préautorisation peut être libérée (troc annulé).
pub fn peut_annuler_paiement(statut: &str) -> bool {
    matches!(statut, "en_attente" | "echoue" | "sequestre")
}

/// Le paiement peut expirer (date limite dépassée sans séquestre).
pub fn peut_expirer(statut: &str) -> bool {
    peut_tenter_preautorisation(statut)
}

/// Un numéro de carte plausible : 12 à 19 chiffres (les espaces sont ignorés
/// par l'appelant). La validité réelle est l'affaire du PSP.
pub fn carte_plausible(digits: &str) -> bool {
    (12..=19).contains(&digits.len()) && digits.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_payeur_suit_la_direction_de_la_soulte() {
        assert_eq!(payeur("du_proposant"), Some(Payeur::Proposant));
        assert_eq!(payeur("du_destinataire"), Some(Payeur::Destinataire));
        assert_eq!(payeur("aucune"), None);
        assert_eq!(payeur("n_importe_quoi"), None);
    }

    #[test]
    fn delai_court_si_le_payeur_accepte_lui_meme() {
        assert_eq!(delai_paiement_minutes(true), 30);
        assert_eq!(delai_paiement_minutes(false), 24 * 60);
    }

    #[test]
    fn commission_en_basis_points_et_net() {
        // Bêta : 0 bps → 0 commission, le bénéficiaire reçoit tout.
        assert_eq!(commission_cents(3_000, 0), 0);
        assert_eq!(net_beneficiaire_cents(3_000, 0), 3_000);
        // 500 bps = 5 % de 30 € → 1,50 €.
        assert_eq!(commission_cents(3_000, 500), 150);
        assert_eq!(net_beneficiaire_cents(3_000, 150), 2_850);
        // Arrondi vers le bas, jamais de net négatif.
        assert_eq!(commission_cents(999, 500), 49);
        assert_eq!(net_beneficiaire_cents(100, 150), 0);
    }

    #[test]
    fn machine_a_etats_paiement() {
        for statut in ["en_attente", "echoue"] {
            assert!(peut_tenter_preautorisation(statut), "{statut}");
            assert!(peut_sequestrer(statut), "{statut}");
            assert!(peut_expirer(statut), "{statut}");
            assert!(peut_annuler_paiement(statut), "{statut}");
            assert!(!peut_capturer(statut), "{statut}");
        }
        assert!(peut_capturer("sequestre"));
        assert!(peut_annuler_paiement("sequestre"));
        assert!(!peut_expirer("sequestre"));
        assert!(!peut_sequestrer("sequestre"));
        for statut in ["capture", "annule", "expire"] {
            assert!(!peut_tenter_preautorisation(statut), "{statut}");
            assert!(!peut_capturer(statut), "{statut}");
            assert!(!peut_annuler_paiement(statut), "{statut}");
            assert!(!peut_expirer(statut), "{statut}");
        }
    }

    #[test]
    fn carte_plausible_sur_la_longueur() {
        assert!(carte_plausible("4970000000000000"));
        assert!(carte_plausible("497000000002")); // 12 chiffres
        assert!(!carte_plausible("49700000000")); // 11
        assert!(!carte_plausible("49700000000000000000")); // 20
        assert!(!carte_plausible("4970abc000000000"));
        assert!(!carte_plausible(""));
    }
}
