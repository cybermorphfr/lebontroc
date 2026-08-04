/** Libellés et formats partagés du catalogue. */

export const CONDITION_LABELS: Record<string, string> = {
  neuf: "Neuf",
  tres_bon_etat: "Très bon état",
  bon_etat: "Bon état",
  correct: "Correct",
};

export const DELIVERY_LABELS: Record<string, string> = {
  main_propre: "Remise en main propre",
  envoi: "Envoi possible",
  les_deux: "Main propre ou envoi",
};

/** « Vient d'arriver » ou « Troque depuis mars 2026 ». */
export function ancrage(memberSince: string): string {
  const date = new Date(memberSince);
  if (Date.now() - date.getTime() < 30 * 24 * 3600 * 1000) return "Vient d'arriver";
  const mois = new Intl.DateTimeFormat("fr-FR", { month: "long", year: "numeric" }).format(date);
  return `Troque depuis ${mois}`;
}

/** Distance approximative lisible — jamais de précision trompeuse. */
export function distanceLabel(km: number): string {
  if (km < 1) return "tout près";
  return `à ${Math.round(km)} km`;
}
