//! Règles pures de l'envoi croisé (F4.3) — sans IO. Formats forfaitaires
//! (jamais de pesée, standard C2C), machine à états par colis, fenêtre de
//! confirmation après retrait, échéances de dépôt.

/// Formats de colis : (code, plafond en kg, transport en centimes).
/// Barème calé sur les points relais France 2026 (Mondial Relay, Shop2Shop).
pub const FORMATS: [(&str, u32, i32); 3] = [("s", 1, 450), ("m", 3, 690), ("l", 10, 990)];

/// Frais de service Lebontroc par colis, en centimes (ligne séparée).
pub const SERVICE_CENTS: i32 = 200;

/// Le transport d'un format, si le format existe.
pub fn transport_cents(format: &str) -> Option<i32> {
    FORMATS
        .iter()
        .find(|(code, _, _)| *code == format)
        .map(|(_, _, cents)| *cents)
}

/// Ce qu'une partie paie en mode envoi : le transport de SON colis, les
/// frais de service, et sa soulte éventuelle.
pub fn montant_payeur_cents(transport_cents: i32, soulte_cents: i32) -> i32 {
    transport_cents + SERVICE_CENTS + soulte_cents
}

/// Statuts d'un colis. Le simulateur de la bêta traverse `depose` →
/// `arrive` instantanément ; le transporteur réel avancera aux webhooks.
pub const STATUTS_COLIS: [&str; 9] = [
    "preparation", // format/relais pas encore complets
    "etiquette",   // étiquette générée, à déposer
    "depose",
    "transit",
    "arrive", // au point relais de destination
    "retire", // récupéré — la fenêtre de confirmation démarre ICI
    "confirme",
    "incident",
    "annule",
];

/// Rang d'un statut dans la progression nominale (`None` pour les
/// terminaux hors parcours). Sert à ignorer les régressions : un webhook
/// en retard ne fait jamais reculer un colis.
pub fn rang_statut(statut: &str) -> Option<u8> {
    match statut {
        "preparation" => Some(0),
        "etiquette" => Some(1),
        "depose" => Some(2),
        "transit" => Some(3),
        "arrive" => Some(4),
        "retire" => Some(5),
        "confirme" => Some(6),
        _ => None,
    }
}

/// Une transition de suivi est-elle une avancée légitime ?
pub fn peut_avancer(depuis: &str, vers: &str) -> bool {
    match (rang_statut(depuis), rang_statut(vers)) {
        (Some(d), Some(v)) => v > d,
        _ => false,
    }
}

/// Fenêtre de signalement après le retrait en relais : passée sans
/// problème signalé, le colis est confirmé automatiquement.
pub const CONFIRMATION_HEURES: i64 = 72;

/// Rappels de dépôt (jours après l'acceptation) puis échec.
pub const RAPPEL_DEPOT_JOURS: [i64; 2] = [2, 4];
/// Non-dépôt à J+5 : le troc échoue (annulation ou litige gelé).
pub const ECHEC_DEPOT_JOURS: i64 = 5;
/// Filet de sécurité : un troc envoi encore ouvert à J+21 est gelé pour
/// examen manuel (colis jamais retiré, situation anormale).
pub const GEL_TROC_ENVOI_JOURS: i64 = 21;

/// Statut du troc dérivé des deux colis (fonction pure, jamais stockée en
/// doublon) : `Some("finalise")` quand les deux sont confirmés,
/// `Some("litige_gele")` dès qu'un incident est signalé, `None` sinon.
pub fn statut_troc_derive(colis_aller: &str, colis_retour: &str) -> Option<&'static str> {
    if colis_aller == "incident" || colis_retour == "incident" {
        Some("litige_gele")
    } else if colis_aller == "confirme" && colis_retour == "confirme" {
        Some("finalise")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bareme_des_formats() {
        assert_eq!(transport_cents("s"), Some(450));
        assert_eq!(transport_cents("m"), Some(690));
        assert_eq!(transport_cents("l"), Some(990));
        assert_eq!(transport_cents("xl"), None);
        assert_eq!(transport_cents(""), None);
    }

    #[test]
    fn montant_du_payeur() {
        // Format M sans soulte : 6,90 + 2,00 = 8,90 €.
        assert_eq!(montant_payeur_cents(690, 0), 890);
        // Format S avec 20 € de soulte : 4,50 + 2,00 + 20,00 = 26,50 €.
        assert_eq!(montant_payeur_cents(450, 2_000), 2_650);
    }

    #[test]
    fn la_machine_a_etats_n_avance_que_vers_l_avant() {
        assert!(peut_avancer("etiquette", "depose"));
        assert!(peut_avancer("depose", "arrive")); // le simulateur saute transit
        assert!(peut_avancer("arrive", "retire"));
        assert!(peut_avancer("retire", "confirme"));
        // Régressions et terminaux : jamais.
        assert!(!peut_avancer("arrive", "depose"));
        assert!(!peut_avancer("confirme", "retire"));
        assert!(!peut_avancer("incident", "confirme"));
        assert!(!peut_avancer("annule", "depose"));
        assert!(!peut_avancer("depose", "incident")); // incident = décision app, pas suivi
    }

    #[test]
    fn statut_du_troc_derive_des_deux_colis() {
        assert_eq!(statut_troc_derive("confirme", "confirme"), Some("finalise"));
        assert_eq!(statut_troc_derive("confirme", "retire"), None);
        assert_eq!(statut_troc_derive("depose", "arrive"), None);
        assert_eq!(
            statut_troc_derive("incident", "confirme"),
            Some("litige_gele")
        );
        assert_eq!(
            statut_troc_derive("depose", "incident"),
            Some("litige_gele")
        );
    }
}
