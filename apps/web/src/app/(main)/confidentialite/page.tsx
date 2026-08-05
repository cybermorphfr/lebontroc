export const metadata = { title: "Confidentialité — Lebontroc" };

/** Politique de confidentialité (F6.3). */
export default function ConfidentialitePage() {
  return (
    <main className="mx-auto flex w-full max-w-2xl flex-col gap-4 px-6 pb-16">
      <h1 className="font-display text-3xl">Politique de confidentialité</h1>
      <p className="text-xs text-neutre-700">Version bêta — 5 août 2026</p>
      {[
        ["Ce qu'on collecte", "Ton e-mail, ton pseudo, ton code postal (jamais affiché en entier : seule ta commune apparaît), tes annonces et photos, tes messages, et les données nécessaires aux trocs (règlements, colis, évaluations, litiges)."],
        ["Ce qu'on en fait", "Faire fonctionner le service, et rien d'autre : mise en relation locale, sécurisation des soultes, résolution des litiges, notifications (réglables), e-mails transactionnels. Aucune revente, aucune publicité ciblée."],
        ["Mesure d'audience", "Nos statistiques d'usage sont pseudonymisées (ton identifiant est remplacé par une empreinte irréversible) et servent uniquement à améliorer le produit. Pas de traceur publicitaire, pas de cookies tiers."],
        ["Où et combien de temps", "Les données sont hébergées en France. Les notifications sont purgées après 90 jours, les pièces de litige restent privées (accès signé), les sessions expirent d'elles-mêmes."],
        ["Tes droits", "Depuis ton profil : exporter toutes tes données (JSON) ou supprimer ton compte. La suppression anonymise ton profil et retire tes objets ; les trocs finalisés avec soulte sont conservés sous forme anonymisée (obligations comptables, 10 ans). Pour toute demande : la boîte de contact indiquée dans les mentions légales."],
      ].map(([titre, corps]) => (
        <section key={titre} className="flex flex-col gap-1">
          <h2 className="font-display text-lg">{titre}</h2>
          <p className="text-sm text-neutre-700">{corps}</p>
        </section>
      ))}
    </main>
  );
}
