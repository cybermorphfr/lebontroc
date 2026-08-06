//! Rôles d'administration et règles d'habilitation (sans IO).
//!
//! Trois niveaux : `utilisateur` (aucun accès), `admin` (traitement
//! quotidien : litiges, signalements, journal) et `super_admin` (actions
//! sensibles : rôles, sanctions, recherche transverse, indicateurs).
//! Le compte maître est un super-admin que personne d'autre ne peut
//! rétrograder ni sanctionner.

pub const ROLE_UTILISATEUR: &str = "utilisateur";
pub const ROLE_ADMIN: &str = "admin";
pub const ROLE_SUPER_ADMIN: &str = "super_admin";

pub const ROLES: [&str; 3] = [ROLE_UTILISATEUR, ROLE_ADMIN, ROLE_SUPER_ADMIN];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminError {
    /// Rôle inconnu.
    RoleInvalide,
    /// L'auteur n'a pas le niveau requis.
    NiveauInsuffisant,
    /// Le compte maître ne se touche pas.
    CompteMaitre,
    /// On ne modifie pas son propre rôle (anti-verrouillage et anti-escalade).
    SoiMeme,
}

pub fn role_valide(role: &str) -> bool {
    ROLES.contains(&role)
}

/// Accès au panneau d'administration (lecture et traitement courant).
pub fn peut_administrer(role: &str) -> bool {
    role == ROLE_ADMIN || role == ROLE_SUPER_ADMIN
}

/// Actions sensibles : gestion des rôles, sanctions, vue globale.
pub fn est_super_admin(role: &str) -> bool {
    role == ROLE_SUPER_ADMIN
}

/// Peut-on attribuer `nouveau_role` à cette cible ?
///
/// Seul un super-admin promeut ou rétrograde. Le compte maître est
/// intouchable, et personne ne change son propre rôle — sans quoi un
/// super-admin pourrait se rétrograder par erreur et laisser la
/// plateforme sans pilote, ou un admin s'auto-promouvoir.
pub fn peut_changer_role(
    role_acteur: &str,
    acteur_est_la_cible: bool,
    cible_est_maitre: bool,
    nouveau_role: &str,
) -> Result<(), AdminError> {
    if !est_super_admin(role_acteur) {
        return Err(AdminError::NiveauInsuffisant);
    }
    if !role_valide(nouveau_role) {
        return Err(AdminError::RoleInvalide);
    }
    if cible_est_maitre {
        return Err(AdminError::CompteMaitre);
    }
    if acteur_est_la_cible {
        return Err(AdminError::SoiMeme);
    }
    Ok(())
}

/// Peut-on sanctionner (bannir, restreindre, lever) ce compte ?
pub fn peut_sanctionner(role_acteur: &str, cible_est_maitre: bool) -> Result<(), AdminError> {
    if !est_super_admin(role_acteur) {
        return Err(AdminError::NiveauInsuffisant);
    }
    if cible_est_maitre {
        return Err(AdminError::CompteMaitre);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchie_des_acces() {
        assert!(!peut_administrer(ROLE_UTILISATEUR));
        assert!(peut_administrer(ROLE_ADMIN));
        assert!(peut_administrer(ROLE_SUPER_ADMIN));
        assert!(!est_super_admin(ROLE_ADMIN));
        assert!(est_super_admin(ROLE_SUPER_ADMIN));
    }

    #[test]
    fn seul_un_super_admin_change_les_roles() {
        assert_eq!(
            peut_changer_role(ROLE_ADMIN, false, false, ROLE_ADMIN),
            Err(AdminError::NiveauInsuffisant)
        );
        assert_eq!(
            peut_changer_role(ROLE_UTILISATEUR, false, false, ROLE_ADMIN),
            Err(AdminError::NiveauInsuffisant)
        );
        assert!(peut_changer_role(ROLE_SUPER_ADMIN, false, false, ROLE_ADMIN).is_ok());
    }

    #[test]
    fn le_compte_maitre_est_intouchable() {
        assert_eq!(
            peut_changer_role(ROLE_SUPER_ADMIN, false, true, ROLE_UTILISATEUR),
            Err(AdminError::CompteMaitre)
        );
        assert_eq!(
            peut_sanctionner(ROLE_SUPER_ADMIN, true),
            Err(AdminError::CompteMaitre)
        );
        assert!(peut_sanctionner(ROLE_SUPER_ADMIN, false).is_ok());
    }

    #[test]
    fn on_ne_change_pas_son_propre_role() {
        assert_eq!(
            peut_changer_role(ROLE_SUPER_ADMIN, true, false, ROLE_UTILISATEUR),
            Err(AdminError::SoiMeme)
        );
    }

    #[test]
    fn role_inconnu_refuse() {
        assert_eq!(
            peut_changer_role(ROLE_SUPER_ADMIN, false, false, "dieu"),
            Err(AdminError::RoleInvalide)
        );
    }
}
