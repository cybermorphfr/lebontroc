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

export const STATUS_PROPOSITION: Record<
  string,
  { label: string; variant: "accent" | "accent-2" | "neutral" }
> = {
  envoyee: { label: "Envoyée", variant: "accent-2" },
  vue: { label: "Vue", variant: "accent" },
  acceptee: { label: "Acceptée", variant: "accent-2" },
  refusee: { label: "Refusée", variant: "neutral" },
  contre_proposee: { label: "Contre-proposée", variant: "accent" },
  expiree: { label: "Expirée", variant: "neutral" },
  caduque: { label: "Caduque", variant: "neutral" },
};

/** Fraîcheur relative façon Leboncoin : « il y a 2 h », « hier », « il y a 5 j ». */
export function timeAgo(iso: string): string {
  const minutes = Math.max(0, Math.floor((Date.now() - new Date(iso).getTime()) / 60000));
  if (minutes < 60) return minutes <= 1 ? "à l'instant" : `il y a ${minutes} min`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `il y a ${hours} h`;
  const days = Math.floor(hours / 24);
  if (days === 1) return "hier";
  if (days < 30) return `il y a ${days} j`;
  return `il y a ${Math.floor(days / 30)} mois`;
}
