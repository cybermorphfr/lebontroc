//! Règles pures d'inscription et de connexion — sans IO.

/// Erreurs de validation des champs d'inscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationError {
    EmailInvalide,
    MotDePasseTropCourt,
    PseudoInvalide,
    CodePostalInvalide,
}

pub const MOT_DE_PASSE_LONGUEUR_MIN: usize = 8;
pub const VERROU_SEUIL_ECHECS: i16 = 5;
pub const VERROU_DUREE_MINUTES: i64 = 15;

/// Validation minimale d'e-mail : quelque chose @ quelque chose . quelque chose.
pub fn valider_email(email: &str) -> Result<(), ValidationError> {
    let email = email.trim();
    let Some((local, domaine)) = email.split_once('@') else {
        return Err(ValidationError::EmailInvalide);
    };
    let domaine_ok = domaine.contains('.')
        && !domaine.starts_with('.')
        && !domaine.ends_with('.')
        && domaine.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if local.is_empty() || !domaine_ok || email.contains(' ') || email.len() > 254 {
        return Err(ValidationError::EmailInvalide);
    }
    Ok(())
}

pub fn valider_mot_de_passe(mot_de_passe: &str) -> Result<(), ValidationError> {
    if mot_de_passe.chars().count() < MOT_DE_PASSE_LONGUEUR_MIN {
        return Err(ValidationError::MotDePasseTropCourt);
    }
    Ok(())
}

/// Pseudo : 3–30 caractères, lettres, chiffres, tirets ou underscore.
pub fn valider_pseudo(pseudo: &str) -> Result<(), ValidationError> {
    let longueur = pseudo.chars().count();
    if longueur < 3 || longueur > 30 {
        return Err(ValidationError::PseudoInvalide);
    }
    if !pseudo
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ValidationError::PseudoInvalide);
    }
    Ok(())
}

/// Code postal français : exactement 5 chiffres.
pub fn valider_code_postal(code_postal: &str) -> Result<(), ValidationError> {
    if code_postal.len() != 5 || !code_postal.chars().all(|c| c.is_ascii_digit()) {
        return Err(ValidationError::CodePostalInvalide);
    }
    Ok(())
}

/// Règle de verrouillage : au N-ième échec consécutif, le compte est verrouillé.
pub fn doit_verrouiller(echecs_consecutifs: i16) -> bool {
    echecs_consecutifs >= VERROU_SEUIL_ECHECS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_valide_et_invalides() {
        assert!(valider_email("camille@exemple.fr").is_ok());
        assert!(valider_email("a@b.co").is_ok());
        assert!(valider_email("sans-arobase.fr").is_err());
        assert!(valider_email("@exemple.fr").is_err());
        assert!(valider_email("camille@exemple").is_err());
        assert!(valider_email("camille@exem ple.fr").is_err());
    }

    #[test]
    fn mot_de_passe_huit_caracteres_minimum() {
        assert!(valider_mot_de_passe("1234567").is_err());
        assert!(valider_mot_de_passe("12345678").is_ok());
    }

    #[test]
    fn pseudo_regles() {
        assert!(valider_pseudo("camille_troc").is_ok());
        assert!(valider_pseudo("ab").is_err());
        assert!(valider_pseudo("a".repeat(31).as_str()).is_err());
        assert!(valider_pseudo("camille troc").is_err());
        assert!(valider_pseudo("camille!").is_err());
    }

    #[test]
    fn code_postal_cinq_chiffres() {
        assert!(valider_code_postal("44000").is_ok());
        assert!(valider_code_postal("4400").is_err());
        assert!(valider_code_postal("4400a").is_err());
        assert!(valider_code_postal("440000").is_err());
    }

    #[test]
    fn verrouillage_au_cinquieme_echec() {
        assert!(!doit_verrouiller(4));
        assert!(doit_verrouiller(5));
        assert!(doit_verrouiller(6));
    }
}
