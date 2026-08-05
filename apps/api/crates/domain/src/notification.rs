//! Taxonomie des notifications (F5.3) — liste fermée, figée avec Brian le
//! 2026-08-05. L'in-app est systématique ; seul le canal e-mail se règle,
//! et uniquement pour les types non critiques.

/// Types de notification utilisateur (les alertes admin restent hors
/// taxonomie : e-mail brut vers ADMIN_EMAIL).
pub const TYPES: &[&str] = &[
    "proposition_recue",
    "proposition_acceptee",
    "proposition_cloturee",
    "message_recu",
    "paiement",
    "expedition",
    "remise",
    "evaluation",
    "litige",
    "favori",
];

/// Les seuls types dont l'e-mail est désactivable. Tout ce qui touche à
/// l'argent, au contrat en cours ou au juridique est verrouillé.
pub const TYPES_EMAIL_DESACTIVABLES: &[&str] = &[
    "proposition_recue",
    "proposition_cloturee",
    "message_recu",
    "evaluation",
    "favori",
];

/// Purge des notifications au-delà de 90 jours (disque VPS compté).
pub const RETENTION_JOURS: i64 = 90;

pub fn type_valide(kind: &str) -> bool {
    TYPES.contains(&kind)
}

pub fn email_desactivable(kind: &str) -> bool {
    TYPES_EMAIL_DESACTIVABLES.contains(&kind)
}

/// L'e-mail part-il pour ce type, selon les préférences JSON de
/// l'utilisateur ? `{type: false}` = coupé ; clé absente = activé
/// (opt-out, choix Brian — y compris `favori`). Les types verrouillés
/// ignorent les préférences.
pub fn email_active(prefs: &serde_json::Value, kind: &str) -> bool {
    if !email_desactivable(kind) {
        return true;
    }
    prefs.get(kind).and_then(|v| v.as_bool()).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn les_types_critiques_ignorent_les_preferences() {
        let prefs = json!({"paiement": false, "litige": false});
        assert!(email_active(&prefs, "paiement"));
        assert!(email_active(&prefs, "litige"));
        assert!(email_active(&prefs, "expedition"));
    }

    #[test]
    fn gherkin_favoris_coupables() {
        // Défaut : activé (opt-out).
        assert!(email_active(&json!({}), "favori"));
        // Coupé : plus d'e-mail — l'in-app reste (hors de ce module).
        assert!(!email_active(&json!({"favori": false}), "favori"));
        // Rallumé explicitement.
        assert!(email_active(&json!({"favori": true}), "favori"));
    }

    #[test]
    fn taxonomie_fermee() {
        assert!(type_valide("proposition_recue"));
        assert!(!type_valide("spam"));
        for kind in TYPES_EMAIL_DESACTIVABLES {
            assert!(TYPES.contains(kind), "{kind} doit être un type connu");
        }
    }
}
