/** Briques d'interface du panneau — le design system, décliné pour l'admin. */

export function Carte({
  titre,
  children,
}: {
  titre?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="flex flex-col gap-3 rounded-[28px] bg-sable p-5 shadow-sm">
      {titre ? <h2 className="font-display text-lg">{titre}</h2> : null}
      {children}
    </section>
  );
}

export function Pastille({
  ton,
  children,
}: {
  ton: "attente" | "ok" | "alerte" | "neutre";
  children: React.ReactNode;
}) {
  const tons = {
    attente: "bg-terracotta-100 text-terracotta-800",
    ok: "bg-sauge-100 text-sauge-800",
    alerte: "bg-terracotta-100 text-terracotta-800 font-bold",
    neutre: "bg-neutre-100 text-neutre-700",
  } as const;
  return (
    <span
      className={`inline-flex items-center whitespace-nowrap rounded-full px-2.5 py-0.5 text-[11px] font-semibold ${tons[ton]}`}
    >
      {children}
    </span>
  );
}

export function BoutonPrimaire(props: React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      {...props}
      className="flex min-h-10 cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600 disabled:cursor-not-allowed disabled:opacity-50"
    />
  );
}

export function BoutonSecondaire(props: React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      {...props}
      className="flex min-h-10 cursor-pointer items-center justify-center rounded-full border border-neutre-300 px-4 text-sm text-encre transition-colors hover:bg-encre/7"
    />
  );
}

export const champ =
  "rounded-full border border-neutre-300 bg-creme px-3.5 py-2 text-sm outline-none transition-colors focus:border-terracotta-500";
export const selecteur =
  "rounded-full border border-neutre-300 bg-creme px-3 py-2 text-sm outline-none";

export function Statistique({ valeur, libelle }: { valeur: string | number; libelle: string }) {
  return (
    <div className="flex flex-col items-center gap-0.5 rounded-3xl bg-creme px-4 py-3">
      <span className="font-display text-2xl text-terracotta-800">{valeur}</span>
      <span className="text-center text-[11px] text-neutre-700">{libelle}</span>
    </div>
  );
}

/** Mini-courbe SVG des 30 derniers jours — rendue côté serveur, zéro lib. */
export function Sparkline({ valeurs, libelle }: { valeurs: number[]; libelle: string }) {
  const largeur = 120;
  const hauteur = 32;
  const max = Math.max(...valeurs, 1);
  const pas = valeurs.length > 1 ? largeur / (valeurs.length - 1) : largeur;
  const points = valeurs
    .map((v, i) => `${(i * pas).toFixed(1)},${(hauteur - (v / max) * (hauteur - 4) - 2).toFixed(1)}`)
    .join(" ");
  const total = valeurs.reduce((a, b) => a + b, 0);
  return (
    <div className="flex flex-col items-center gap-1 rounded-3xl bg-creme px-4 py-3">
      <span className="font-display text-xl text-terracotta-800">{total}</span>
      <svg
        viewBox={`0 0 ${largeur} ${hauteur}`}
        className="h-8 w-full max-w-[120px]"
        aria-hidden="true"
      >
        <polyline
          points={points}
          fill="none"
          stroke="#c67139"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
      <span className="text-center text-[11px] text-neutre-700">{libelle}</span>
    </div>
  );
}

/** Variation hebdomadaire : flèche + pourcentage, colorée selon le sens.
 * `inverse` pour les métriques où la hausse est une mauvaise nouvelle
 * (litiges, échecs). */
export function Variation({
  actuel,
  precedent,
  inverse = false,
}: {
  actuel: number;
  precedent: number;
  inverse?: boolean;
}) {
  if (precedent === 0) {
    return <Pastille ton="neutre">{actuel > 0 ? "nouveau" : "—"}</Pastille>;
  }
  const pct = Math.round(((actuel - precedent) / precedent) * 100);
  if (pct === 0) return <Pastille ton="neutre">stable</Pastille>;
  const bonne = inverse ? pct < 0 : pct > 0;
  return (
    <Pastille ton={bonne ? "ok" : "attente"}>
      {pct > 0 ? "↗" : "↘"} {pct > 0 ? "+" : ""}
      {pct} %
    </Pastille>
  );
}
