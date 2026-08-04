//! Règles pures du troc — sans IO. C'est ici que vivent le plafond de
//! soulte et la machine à états des propositions (backlog §architecture).

/// Erreurs métier d'une proposition de troc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeError {
    /// Il faut au moins un objet de chaque côté.
    ObjetsManquants,
    /// Trop d'objets d'un côté (garde-fou anti-abus).
    TropDObjets,
    /// La soulte dépasse le plafond autorisé (valeur = plafond en centimes).
    SoulteTropHaute(i32),
    /// Une soulte sans direction (ou l'inverse) n'a pas de sens.
    SoulteIncoherente,
    /// Transition de statut interdite.
    TransitionInterdite,
}

pub const OBJETS_MAX_PAR_COTE: usize = 10;
/// Durée de vie d'une proposition sans réponse (Gherkin F3.1 : 7 jours).
pub const EXPIRATION_JOURS: i64 = 7;

/// Plafond de soulte : 50 % de la valeur indicative du meilleur objet de la
/// proposition, tous côtés confondus (Gherkin : meilleur objet 100 € → 50 €).
pub fn plafond_soulte_cents(valeurs_cents: &[i32]) -> i32 {
    valeurs_cents.iter().copied().max().unwrap_or(0) / 2
}

/// Directions possibles d'une soulte.
pub const DIRECTIONS_SOULTE: [&str; 3] = ["aucune", "du_proposant", "du_destinataire"];

/// Valide la composition d'une proposition : au moins un objet par côté,
/// pas trop d'objets, soulte cohérente et sous le plafond.
pub fn valider_proposition(
    valeurs_offertes_cents: &[i32],
    valeurs_demandees_cents: &[i32],
    cash_cents: i32,
    cash_direction: &str,
) -> Result<(), TradeError> {
    if valeurs_offertes_cents.is_empty() || valeurs_demandees_cents.is_empty() {
        return Err(TradeError::ObjetsManquants);
    }
    if valeurs_offertes_cents.len() > OBJETS_MAX_PAR_COTE
        || valeurs_demandees_cents.len() > OBJETS_MAX_PAR_COTE
    {
        return Err(TradeError::TropDObjets);
    }
    if !DIRECTIONS_SOULTE.contains(&cash_direction) {
        return Err(TradeError::SoulteIncoherente);
    }
    match (cash_cents, cash_direction) {
        (0, "aucune") => Ok(()),
        (0, _) | (_, "aucune") => Err(TradeError::SoulteIncoherente),
        (cash, _) if cash < 0 => Err(TradeError::SoulteIncoherente),
        (cash, _) => {
            let toutes: Vec<i32> = valeurs_offertes_cents
                .iter()
                .chain(valeurs_demandees_cents)
                .copied()
                .collect();
            let plafond = plafond_soulte_cents(&toutes);
            if cash > plafond {
                Err(TradeError::SoulteTropHaute(plafond))
            } else {
                Ok(())
            }
        }
    }
}

/// Le destinataire peut refuser une proposition encore ouverte.
pub fn peut_refuser(statut: &str) -> Result<(), TradeError> {
    match statut {
        "envoyee" | "vue" => Ok(()),
        _ => Err(TradeError::TransitionInterdite),
    }
}

/// Le destinataire peut accepter une proposition encore ouverte.
pub fn peut_accepter(statut: &str) -> Result<(), TradeError> {
    peut_refuser(statut)
}

/// Le destinataire peut contre-proposer sur une proposition encore ouverte.
pub fn peut_contre_proposer(statut: &str) -> Result<(), TradeError> {
    peut_refuser(statut)
}

/// Modes de remise possibles d'un troc (choisis à l'acceptation).
pub fn valider_mode_remise(mode: &str) -> Result<(), TradeError> {
    match mode {
        "main_propre" | "envoi" => Ok(()),
        _ => Err(TradeError::TransitionInterdite),
    }
}

/// Une proposition passe à `vue` uniquement depuis `envoyee`.
pub fn peut_marquer_vue(statut: &str) -> bool {
    statut == "envoyee"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plafond_moitie_du_meilleur_objet() {
        // Gherkin : meilleur objet 100 € → plafond 50 €.
        assert_eq!(plafond_soulte_cents(&[10_000, 4_000]), 5_000);
        assert_eq!(plafond_soulte_cents(&[]), 0);
    }

    #[test]
    fn scenario_console_plus_30_contre_velo() {
        // Gherkin : console 120 € + 30 € contre vélo 150 € → acceptée
        // (plafond = 150 / 2 = 75 €).
        assert!(valider_proposition(&[12_000], &[15_000], 3_000, "du_proposant").is_ok());
    }

    #[test]
    fn soulte_au_dela_du_plafond_refusee_avec_le_plafond() {
        let erreur = valider_proposition(&[10_000], &[8_000], 6_000, "du_proposant");
        assert_eq!(erreur, Err(TradeError::SoulteTropHaute(5_000)));
        // Pile au plafond : accepté.
        assert!(valider_proposition(&[10_000], &[8_000], 5_000, "du_proposant").is_ok());
    }

    #[test]
    fn soulte_incoherente() {
        assert_eq!(
            valider_proposition(&[100], &[100], 0, "du_proposant"),
            Err(TradeError::SoulteIncoherente)
        );
        assert_eq!(
            valider_proposition(&[100], &[100], 500, "aucune"),
            Err(TradeError::SoulteIncoherente)
        );
        assert_eq!(
            valider_proposition(&[100], &[100], 500, "vers_la_lune"),
            Err(TradeError::SoulteIncoherente)
        );
        assert_eq!(
            valider_proposition(&[100], &[100], -1, "du_proposant"),
            Err(TradeError::SoulteIncoherente)
        );
    }

    #[test]
    fn au_moins_un_objet_par_cote_et_pas_trop() {
        assert_eq!(
            valider_proposition(&[], &[100], 0, "aucune"),
            Err(TradeError::ObjetsManquants)
        );
        assert_eq!(
            valider_proposition(&[100], &[], 0, "aucune"),
            Err(TradeError::ObjetsManquants)
        );
        let onze = vec![100; 11];
        assert_eq!(
            valider_proposition(&onze, &[100], 0, "aucune"),
            Err(TradeError::TropDObjets)
        );
    }

    #[test]
    fn acceptation_et_contre_sur_proposition_ouverte_seulement() {
        for statut in ["envoyee", "vue"] {
            assert!(peut_accepter(statut).is_ok());
            assert!(peut_contre_proposer(statut).is_ok());
        }
        for statut in [
            "acceptee",
            "refusee",
            "expiree",
            "contre_proposee",
            "caduque",
        ] {
            assert!(peut_accepter(statut).is_err(), "accepter {statut}");
            assert!(peut_contre_proposer(statut).is_err(), "contrer {statut}");
        }
        assert!(valider_mode_remise("main_propre").is_ok());
        assert!(valider_mode_remise("envoi").is_ok());
        assert!(valider_mode_remise("teleportation").is_err());
    }

    #[test]
    fn machine_a_etats_refus_et_vue() {
        assert!(peut_refuser("envoyee").is_ok());
        assert!(peut_refuser("vue").is_ok());
        assert!(peut_refuser("refusee").is_err());
        assert!(peut_refuser("expiree").is_err());
        assert!(peut_refuser("acceptee").is_err());
        assert!(peut_marquer_vue("envoyee"));
        assert!(!peut_marquer_vue("vue"));
        assert!(!peut_marquer_vue("refusee"));
    }
}
