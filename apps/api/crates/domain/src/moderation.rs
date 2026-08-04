//! Modération pure des messages : masquage des coordonnées avant acceptation
//! (téléphone, e-mail, IBAN) — l'indicateur clé du risque de contournement.

use std::sync::LazyLock;

use regex::Regex;

/// Ce qui remplace une coordonnée masquée.
pub const MASQUE: &str = "•••••••";

static TELEPHONE: LazyLock<Regex> = LazyLock::new(|| {
    // 06 12 34 56 78, 06.12.34.56.78, 0612345678, +33 6 12 34 56 78…
    Regex::new(r"(?:\+\s?33[\s.\-]?|0)\s?[1-9](?:[\s.\-]?\d{2}){4}").expect("regex téléphone")
});
static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").expect("regex e-mail")
});
static IBAN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Za-z]{2}\d{2}(?:\s?[A-Za-z0-9]{4}){2,8}(?:\s?[A-Za-z0-9]{1,3})?\b")
        .expect("regex IBAN")
});

/// Masque téléphones, e-mails et IBAN. Retourne le texte nettoyé et `true`
/// si au moins une coordonnée a été masquée.
pub fn masquer_coordonnees(texte: &str) -> (String, bool) {
    let mut masque = false;
    let mut resultat = texte.to_string();
    for pattern in [&*TELEPHONE, &*EMAIL, &*IBAN] {
        if pattern.is_match(&resultat) {
            masque = true;
            resultat = pattern.replace_all(&resultat, MASQUE).into_owned();
        }
    }
    (resultat, masque)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masque_le_telephone_du_gherkin() {
        // Gherkin F3.2 : « appelle-moi au 06 12 34 56 78 ».
        let (texte, masque) = masquer_coordonnees("appelle-moi au 06 12 34 56 78");
        assert!(masque);
        assert_eq!(texte, format!("appelle-moi au {MASQUE}"));
    }

    #[test]
    fn masque_les_variantes_de_telephone() {
        for cas in [
            "0612345678",
            "06.12.34.56.78",
            "06-12-34-56-78",
            "+33 6 12 34 56 78",
            "+33612345678",
        ] {
            let (texte, masque) = masquer_coordonnees(cas);
            assert!(masque, "non masqué : {cas}");
            assert!(!texte.contains("12"), "chiffres visibles : {texte}");
        }
    }

    #[test]
    fn masque_email_et_iban() {
        let (texte, masque) = masquer_coordonnees("écris-moi sur jean.dupont@exemple.fr !");
        assert!(masque);
        assert!(!texte.contains("exemple.fr"));

        let (texte, masque) = masquer_coordonnees("vire sur FR76 3000 6000 0112 3456 7890 189");
        assert!(masque);
        assert!(!texte.contains("3456"));
    }

    #[test]
    fn laisse_les_messages_innocents() {
        for cas in [
            "Ton vélo est superbe, on échange ?",
            "Je propose 30 € de soulte en plus.",
            "Rendez-vous samedi vers 14h30 ?",
            "Il mesure 120 cm sur 80.",
        ] {
            let (texte, masque) = masquer_coordonnees(cas);
            assert!(!masque, "faux positif : {cas}");
            assert_eq!(texte, cas);
        }
    }
}
