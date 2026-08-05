//! Règles pures des signalements, litiges et du score de fiabilité (F5.2).
//! Référentiel §5.7 reconstitué et figé avec Brian le 2026-08-05.

use chrono::{DateTime, Duration, Utc};

pub const DESCRIPTION_MAX: usize = 1000;
pub const PIECES_MAX: usize = 5;
/// L'autre partie a 72 h pour verser ses pièces au dossier.
pub const REPONSE_HEURES: i64 = 72;
/// Litige possible jusqu'à 48 h après une remise en main propre — la
/// capture des règlements main propre attend donc ce délai (choix Brian).
pub const FENETRE_MAIN_PROPRE_HEURES: i64 = 48;
/// No-show déclarable à partir de J+3 après l'acceptation.
pub const NO_SHOW_JOURS: i64 = 3;

/// Motifs de litige de troc.
pub const MOTIFS_LITIGE: &[&str] = &[
    "non_conforme",
    "abime",
    "manquant",
    "contrefacon",
    "jamais_venu",
];

/// Motifs de signalement par cible.
pub fn motifs_signalement(target_type: &str) -> Option<&'static [&'static str]> {
    match target_type {
        "objet" => Some(&[
            "contrefacon",
            "interdit_vente",
            "annonce_trompeuse",
            "contenu_inapproprie",
            "spam_doublon",
        ]),
        "utilisateur" => Some(&[
            "arnaque_suspectee",
            "comportement_inapproprie",
            "contournement_plateforme",
            "usurpation_faux_profil",
            "autre",
        ]),
        "message" => Some(&[
            "harcelement_insultes",
            "tentative_arnaque",
            "contournement_masquage",
            "contenu_inapproprie",
        ]),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisputeError {
    MotifInconnu,
    DescriptionInvalide,
    TropDePieces,
    /// Hors fenêtre : trop tôt (no-show < J+3) ou trop tard (> 48 h
    /// post-remise, colis déjà confirmé…).
    HorsFenetre,
}

pub fn valider_ouverture(
    reason: &str,
    description: &str,
    pieces: usize,
) -> Result<(), DisputeError> {
    if !MOTIFS_LITIGE.contains(&reason) {
        return Err(DisputeError::MotifInconnu);
    }
    let description = description.trim();
    if description.is_empty() || description.chars().count() > DESCRIPTION_MAX {
        return Err(DisputeError::DescriptionInvalide);
    }
    if pieces > PIECES_MAX {
        return Err(DisputeError::TropDePieces);
    }
    Ok(())
}

/// Fenêtre d'ouverture selon le mode et l'état du troc.
///
/// - envoi, troc `accepte` : dès qu'un colis est arrivé, tant que le
///   destinataire n'a pas confirmé (confirmer = accepter l'état) ;
/// - main propre, troc `accepte` : `jamais_venu` seulement, à J+3 ;
/// - main propre, troc `finalise` : vice découvert sous 48 h.
pub fn fenetre_ouverture(
    reason: &str,
    delivery_mode: &str,
    trade_status: &str,
    accepted_at: DateTime<Utc>,
    finalized_at: Option<DateTime<Utc>>,
    incoming_receivable: bool,
    now: DateTime<Utc>,
) -> Result<(), DisputeError> {
    let ok = match (delivery_mode, trade_status) {
        ("envoi", "accepte") => reason != "jamais_venu" && incoming_receivable,
        ("main_propre", "accepte") => {
            reason == "jamais_venu" && now >= accepted_at + Duration::days(NO_SHOW_JOURS)
        }
        ("main_propre", "finalise") => {
            reason != "jamais_venu"
                && finalized_at
                    .is_some_and(|f| now < f + Duration::hours(FENETRE_MAIN_PROPRE_HEURES))
        }
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(DisputeError::HorsFenetre)
    }
}

/// La capture d'un règlement main propre attend la fin de la fenêtre.
pub fn capture_main_propre_due(finalized_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    now >= finalized_at + Duration::hours(FENETRE_MAIN_PROPRE_HEURES)
}

// ————— Score de fiabilité interne (jamais public) —————

/// Points par événement négatif du journal `dispute_events`.
pub fn points(event_type: &str) -> i32 {
    match event_type {
        "contrefacon_averee" => 15,
        "litige_perdu" => 6,
        "non_depot" => 5,
        "no_show_confirme" => 4,
        "litige_abusif" => 2,
        "signalement_fonde" => 2,
        _ => 0,
    }
}

pub const SEUIL_AVERTISSEMENT: i32 = 5;
pub const SEUIL_RESTRICTION: i32 = 10;
pub const SEUIL_BANNISSEMENT: i32 = 15;
/// Durée d'une restriction (plus de nouvelles propositions).
pub const RESTRICTION_JOURS: i64 = 30;

/// Sanction automatique atteinte pour un score donné (choix Brian : les
/// seuils s'appliquent sans intervention humaine, e-mail admin à chaque
/// déclenchement, levée possible via l'admin).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sanction {
    Aucune,
    Avertissement,
    Restriction,
    Bannissement,
}

pub fn sanction_pour_score(score: i32) -> Sanction {
    if score >= SEUIL_BANNISSEMENT {
        Sanction::Bannissement
    } else if score >= SEUIL_RESTRICTION {
        Sanction::Restriction
    } else if score >= SEUIL_AVERTISSEMENT {
        Sanction::Avertissement
    } else {
        Sanction::Aucune
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(h: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 1, 12, 0, 0).unwrap() + Duration::hours(h)
    }

    #[test]
    fn ouverture_valide_motif_description_et_pieces() {
        assert!(valider_ouverture("non_conforme", "Le vélo est cassé.", 3).is_ok());
        assert_eq!(
            valider_ouverture("inconnu", "x", 0),
            Err(DisputeError::MotifInconnu)
        );
        assert_eq!(
            valider_ouverture("abime", "  ", 0),
            Err(DisputeError::DescriptionInvalide)
        );
        assert_eq!(
            valider_ouverture("abime", "ok", 6),
            Err(DisputeError::TropDePieces)
        );
    }

    #[test]
    fn fenetre_envoi_avant_confirmation() {
        let ok = fenetre_ouverture("non_conforme", "envoi", "accepte", t(0), None, true, t(1));
        assert!(ok.is_ok());
        // Colis déjà confirmé (plus rien à recevoir) : trop tard.
        let ko = fenetre_ouverture("non_conforme", "envoi", "accepte", t(0), None, false, t(1));
        assert_eq!(ko, Err(DisputeError::HorsFenetre));
        // Pas de no-show en mode envoi.
        let ko = fenetre_ouverture("jamais_venu", "envoi", "accepte", t(0), None, true, t(1));
        assert_eq!(ko, Err(DisputeError::HorsFenetre));
    }

    #[test]
    fn fenetre_no_show_a_j3() {
        let trop_tot = fenetre_ouverture(
            "jamais_venu",
            "main_propre",
            "accepte",
            t(0),
            None,
            false,
            t(71),
        );
        assert_eq!(trop_tot, Err(DisputeError::HorsFenetre));
        let ok = fenetre_ouverture(
            "jamais_venu",
            "main_propre",
            "accepte",
            t(0),
            None,
            false,
            t(73),
        );
        assert!(ok.is_ok());
    }

    #[test]
    fn fenetre_48h_apres_remise() {
        let ok = fenetre_ouverture(
            "non_conforme",
            "main_propre",
            "finalise",
            t(0),
            Some(t(10)),
            false,
            t(10 + 47),
        );
        assert!(ok.is_ok());
        let trop_tard = fenetre_ouverture(
            "non_conforme",
            "main_propre",
            "finalise",
            t(0),
            Some(t(10)),
            false,
            t(10 + 49),
        );
        assert_eq!(trop_tard, Err(DisputeError::HorsFenetre));
        assert!(!capture_main_propre_due(t(10), t(10 + 47)));
        assert!(capture_main_propre_due(t(10), t(10 + 48)));
    }

    #[test]
    fn bareme_et_seuils() {
        assert_eq!(points("contrefacon_averee"), 15);
        assert_eq!(points("non_depot"), 5);
        assert_eq!(points("truc_inconnu"), 0);
        assert_eq!(sanction_pour_score(4), Sanction::Aucune);
        assert_eq!(sanction_pour_score(5), Sanction::Avertissement);
        assert_eq!(sanction_pour_score(11), Sanction::Restriction);
        assert_eq!(sanction_pour_score(15), Sanction::Bannissement);
    }

    #[test]
    fn motifs_signalement_par_cible() {
        assert!(motifs_signalement("objet")
            .unwrap()
            .contains(&"contrefacon"));
        assert!(motifs_signalement("message")
            .unwrap()
            .contains(&"contournement_masquage"));
        assert!(motifs_signalement("pigeon").is_none());
    }
}
