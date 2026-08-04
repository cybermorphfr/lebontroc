//! Règles pures du catalogue — sans IO.

/// Erreurs de validation d'un objet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogError {
    TitreInvalide,
    DescriptionInvalide,
    ValeurInvalide,
    EtatInconnu,
    RemiseInconnue,
    PhotosInvalides,
    StatutInterdit,
}

pub const VALEUR_MIN_CENTS: i32 = 100;
/// Plafond produit 2 000 € : borne le futur plafond de soulte (50 %, F3.1).
pub const VALEUR_MAX_CENTS: i32 = 200_000;
pub const PHOTOS_MIN: usize = 1;
pub const PHOTOS_MAX: usize = 8;

pub const CONDITIONS: [&str; 4] = ["neuf", "tres_bon_etat", "bon_etat", "correct"];
pub const REMISES: [&str; 3] = ["main_propre", "envoi", "les_deux"];

pub fn valider_titre(titre: &str) -> Result<(), CatalogError> {
    let longueur = titre.trim().chars().count();
    if !(3..=80).contains(&longueur) {
        return Err(CatalogError::TitreInvalide);
    }
    Ok(())
}

pub fn valider_description(description: &str) -> Result<(), CatalogError> {
    let longueur = description.trim().chars().count();
    if !(10..=2000).contains(&longueur) {
        return Err(CatalogError::DescriptionInvalide);
    }
    Ok(())
}

pub fn valider_valeur(value_cents: i32) -> Result<(), CatalogError> {
    if !(VALEUR_MIN_CENTS..=VALEUR_MAX_CENTS).contains(&value_cents) {
        return Err(CatalogError::ValeurInvalide);
    }
    Ok(())
}

pub fn valider_condition(condition: &str) -> Result<(), CatalogError> {
    if CONDITIONS.contains(&condition) {
        Ok(())
    } else {
        Err(CatalogError::EtatInconnu)
    }
}

pub fn valider_remise(remise: &str) -> Result<(), CatalogError> {
    if REMISES.contains(&remise) {
        Ok(())
    } else {
        Err(CatalogError::RemiseInconnue)
    }
}

pub fn valider_nombre_photos(nombre: usize) -> Result<(), CatalogError> {
    if !(PHOTOS_MIN..=PHOTOS_MAX).contains(&nombre) {
        return Err(CatalogError::PhotosInvalides);
    }
    Ok(())
}

/// Transitions de statut autorisées à l'utilisateur : `disponible ↔ masque`
/// uniquement. `reserve` et `troque` appartiennent à la machine à états du
/// troc (F3.3/F4.1) — jamais posables manuellement.
pub fn transition_statut_autorisee(actuel: &str, demande: &str) -> Result<(), CatalogError> {
    match (actuel, demande) {
        (a, d) if a == d => Ok(()),
        ("disponible", "masque") | ("masque", "disponible") => Ok(()),
        _ => Err(CatalogError::StatutInterdit),
    }
}

/// Distance à vol d'oiseau entre deux points (lat, lng) en kilomètres.
/// Précision suffisante pour « à 3 km » — jamais utilisée pour de la géoloc fine.
pub fn haversine_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    const RAYON_TERRE_KM: f64 = 6371.0;
    let (lat1, lng1) = (a.0.to_radians(), a.1.to_radians());
    let (lat2, lng2) = (b.0.to_radians(), b.1.to_radians());
    let dlat = lat2 - lat1;
    let dlng = lng2 - lng1;
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlng / 2.0).sin().powi(2);
    2.0 * RAYON_TERRE_KM * h.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titre_borne() {
        assert!(valider_titre("ab").is_err());
        assert!(valider_titre("Vélo").is_ok());
        assert!(valider_titre(&"a".repeat(81)).is_err());
    }

    #[test]
    fn description_borne() {
        assert!(valider_description("court").is_err());
        assert!(valider_description("Une description correcte.").is_ok());
    }

    #[test]
    fn valeur_bornee_1_a_2000_euros() {
        assert!(valider_valeur(99).is_err());
        assert!(valider_valeur(100).is_ok());
        assert!(valider_valeur(200_000).is_ok());
        assert!(valider_valeur(200_001).is_err());
    }

    #[test]
    fn photos_entre_1_et_8() {
        assert!(valider_nombre_photos(0).is_err());
        assert!(valider_nombre_photos(1).is_ok());
        assert!(valider_nombre_photos(8).is_ok());
        assert!(valider_nombre_photos(9).is_err());
    }

    #[test]
    fn statuts_utilisateur_masque_disponible_seulement() {
        assert!(transition_statut_autorisee("disponible", "masque").is_ok());
        assert!(transition_statut_autorisee("masque", "disponible").is_ok());
        assert!(transition_statut_autorisee("disponible", "disponible").is_ok());
        assert!(transition_statut_autorisee("disponible", "reserve").is_err());
        assert!(transition_statut_autorisee("reserve", "disponible").is_err());
        assert!(transition_statut_autorisee("disponible", "troque").is_err());
        assert!(transition_statut_autorisee("reserve", "masque").is_err());
    }

    #[test]
    fn haversine_nantes_paris_environ_343_km() {
        let nantes = (47.2382, -1.5603);
        let paris = (48.8566, 2.3522);
        let d = haversine_km(nantes, paris);
        assert!((330.0..360.0).contains(&d), "distance inattendue : {d}");
        assert!(haversine_km(nantes, nantes) < 0.001);
    }
}
