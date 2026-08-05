export const metadata = { title: "CGU — Lebontroc" };

/** Conditions générales d'utilisation (F6.3) — bêta. */
export default function CguPage() {
  return (
    <main className="prose-sm mx-auto flex w-full max-w-2xl flex-col gap-4 px-6 pb-16">
      <h1 className="font-display text-3xl">Conditions générales d&apos;utilisation</h1>
      <p className="text-xs text-neutre-700">Version bêta — 5 août 2026</p>
      {[
        ["1. Le service", "Lebontroc est une plateforme de mise en relation entre particuliers pour l'échange (troc) d'objets, avec ou sans compensation financière (« soulte »). Lebontroc n'est ni vendeur, ni acheteur, ni propriétaire des objets échangés : le contrat d'échange est conclu directement entre les utilisateurs."],
        ["2. Compte", "L'inscription est réservée aux personnes majeures. Un seul compte par personne ; les informations fournies doivent être exactes. Tu es responsable de la confidentialité de tes identifiants."],
        ["3. Objets et annonces", "Sont interdits : les objets contrefaits, volés, dangereux, réglementés (armes, médicaments…), le vivant, et tout contenu illicite. La valeur indicative est déclarative et n'engage que toi. Lebontroc peut retirer toute annonce non conforme."],
        ["4. Échanges et soulte", "La soulte éventuelle est bloquée (préautorisation) au moment de l'acceptation et n'est débitée qu'à la bonne fin de l'échange — après la remise (avec un délai de contestation de 48 h en main propre) ou la réception des colis. En bêta, les paiements sont simulés."],
        ["5. Litiges", "En cas de problème, un dossier de litige peut être ouvert depuis la page du troc dans les fenêtres prévues. L'équipe tranche sous 7 jours ; les issues possibles sont le débit, l'annulation des règlements ou le classement. Les comportements frauduleux exposent à un avertissement, une restriction ou un bannissement."],
        ["6. Signalements", "Tout contenu ou comportement illicite peut être signalé depuis les profils et les annonces (loi pour la confiance dans l'économie numérique ; règlement européen sur les services numériques)."],
        ["7. Fiscalité", "Les échanges entre particuliers relevant de la gestion du patrimoine privé ne sont en principe pas imposables. Les soultes perçues peuvent, selon ton activité, relever d'obligations déclaratives (dispositif DAC7 : un récapitulatif annuel sera fourni si les seuils sont atteints). En cas de doute, rapproche-toi de l'administration fiscale."],
        ["8. Responsabilité", "Lebontroc met en relation et fournit des outils (messagerie, séquestre, litiges) mais ne garantit pas la qualité, la conformité ou la disponibilité des objets. Le service est fourni « en l'état » pendant la bêta."],
        ["9. Résiliation", "Tu peux supprimer ton compte à tout moment depuis ton profil (voir la politique de confidentialité pour le sort des données). Lebontroc peut suspendre un compte en cas de violation des présentes conditions."],
      ].map(([titre, corps]) => (
        <section key={titre} className="flex flex-col gap-1">
          <h2 className="font-display text-lg">{titre}</h2>
          <p className="text-sm text-neutre-700">{corps}</p>
        </section>
      ))}
    </main>
  );
}
