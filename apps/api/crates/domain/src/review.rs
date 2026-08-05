//! Règles pures des évaluations (F5.1) — sans IO. La réputation est la
//! monnaie du troc : notes 1 à 5, publication simultanée anti-représailles.

pub const NOTE_MIN: i16 = 1;
pub const NOTE_MAX: i16 = 5;
pub const COMMENTAIRE_MAX: usize = 500;
/// Sans note de l'autre partie, la note seule est publiée à J+14 après la
/// finalisation (Gherkin F5.1).
pub const PUBLICATION_JOURS: i64 = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewError {
    NoteHorsBornes,
    CommentaireTropLong,
    /// On ne note qu'un troc finalisé.
    TrocNonFinalise,
}

pub fn valider_note(rating: i16) -> Result<(), ReviewError> {
    if (NOTE_MIN..=NOTE_MAX).contains(&rating) {
        Ok(())
    } else {
        Err(ReviewError::NoteHorsBornes)
    }
}

pub fn valider_commentaire(comment: &str) -> Result<(), ReviewError> {
    if comment.chars().count() <= COMMENTAIRE_MAX {
        Ok(())
    } else {
        Err(ReviewError::CommentaireTropLong)
    }
}

/// Seul un troc finalisé se note — pas un troc annulé ni gelé.
pub fn peut_noter(trade_status: &str) -> Result<(), ReviewError> {
    if trade_status == "finalise" {
        Ok(())
    } else {
        Err(ReviewError::TrocNonFinalise)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_entre_1_et_5() {
        for rating in 1..=5 {
            assert!(valider_note(rating).is_ok());
        }
        assert_eq!(valider_note(0), Err(ReviewError::NoteHorsBornes));
        assert_eq!(valider_note(6), Err(ReviewError::NoteHorsBornes));
        assert_eq!(valider_note(-1), Err(ReviewError::NoteHorsBornes));
    }

    #[test]
    fn commentaire_500_caracteres_max() {
        assert!(valider_commentaire("").is_ok());
        assert!(valider_commentaire(&"é".repeat(500)).is_ok());
        assert_eq!(
            valider_commentaire(&"é".repeat(501)),
            Err(ReviewError::CommentaireTropLong)
        );
    }

    #[test]
    fn seul_un_troc_finalise_se_note() {
        assert!(peut_noter("finalise").is_ok());
        for statut in ["accepte", "attente_paiement", "annule", "litige_gele"] {
            assert_eq!(peut_noter(statut), Err(ReviewError::TrocNonFinalise));
        }
    }
}
