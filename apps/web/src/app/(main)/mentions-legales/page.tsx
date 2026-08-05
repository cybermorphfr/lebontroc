export const metadata = { title: "Mentions légales — Lebontroc" };

/** Mentions légales (F6.3) — à compléter à la création de la structure. */
export default function MentionsLegalesPage() {
  return (
    <main className="mx-auto flex w-full max-w-2xl flex-col gap-4 px-6 pb-16">
      <h1 className="font-display text-3xl">Mentions légales</h1>
      {[
        ["Éditeur", "Lebontroc — service en version bêta édité par Brian P. (entrepreneur individuel en cours de constitution). Contact : plus.brian1992@gmail.com."],
        ["Hébergement", "Serveur privé virtuel hébergé en Union européenne ; stockage des fichiers en France."],
        ["Médiation", "Conformément aux articles L.611-1 et suivants du Code de la consommation, tout litige de consommation peut être soumis à un médiateur ; les coordonnées du médiateur retenu seront publiées à la sortie de bêta."],
        ["Signalement de contenus", "Tout contenu illicite peut être signalé via les boutons de signalement présents sur les profils et les annonces, ou par e-mail à l'adresse de contact ci-dessus (LCEN art. 6 ; DSA)."],
      ].map(([titre, corps]) => (
        <section key={titre} className="flex flex-col gap-1">
          <h2 className="font-display text-lg">{titre}</h2>
          <p className="text-sm text-neutre-700">{corps}</p>
        </section>
      ))}
    </main>
  );
}
