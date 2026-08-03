//! Statut de santé de l'application, dérivé de l'état de ses dépendances.

use serde::Serialize;

/// État d'une dépendance (base de données, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyStatus {
    Ok,
    Unreachable,
}

/// Statut global de l'API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ok,
    Degraded,
}

/// Règle métier : l'API est `Ok` si et seulement si toutes ses dépendances le sont.
pub fn overall_status(dependencies: &[DependencyStatus]) -> HealthStatus {
    if dependencies.iter().all(|d| *d == DependencyStatus::Ok) {
        HealthStatus::Ok
    } else {
        HealthStatus::Degraded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tout_ok_donne_ok() {
        assert_eq!(
            overall_status(&[DependencyStatus::Ok]),
            HealthStatus::Ok
        );
    }

    #[test]
    fn une_dependance_injoignable_degrade() {
        assert_eq!(
            overall_status(&[DependencyStatus::Ok, DependencyStatus::Unreachable]),
            HealthStatus::Degraded
        );
    }

    #[test]
    fn aucune_dependance_donne_ok() {
        assert_eq!(overall_status(&[]), HealthStatus::Ok);
    }
}
