import Link from "next/link";

export const metadata = {
  title: "Comment ça marche — Lebontroc",
  description:
    "Le guide complet du troc sur Lebontroc : publier, proposer, échanger en main propre ou par colis, en toute sécurité.",
};

// Guide utilisateur intégré (demande Brian) — la référence produit vit
// aussi dans docs/GUIDE-UTILISATEUR.md.

const SOMMAIRE = [
  ["compte", "1. Créer son compte"],
  ["publier", "2. Publier un objet"],
  ["trouver", "3. Trouver son bonheur"],
  ["proposer", "4. Proposer un troc"],
  ["echanger", "5. L'échange"],
  ["evaluer", "6. Les évaluations"],
  ["probleme", "7. Si ça se passe mal"],
  ["notifications", "8. Les notifications"],
  ["donnees", "9. Tes données"],
] as const;

function Section({
  id,
  titre,
  emoji,
  children,
}: {
  id: string;
  titre: string;
  emoji: string;
  children: React.ReactNode;
}) {
  return (
    <section
      id={id}
      className="flex scroll-mt-24 flex-col gap-3 rounded-[32px] bg-sable p-6 shadow-sm sm:p-8"
    >
      <h2 className="font-display text-2xl">
        <span aria-hidden className="mr-2">
          {emoji}
        </span>
        {titre}
      </h2>
      <div className="flex flex-col gap-3 text-sm leading-relaxed text-neutre-700">
        {children}
      </div>
    </section>
  );
}

function Etapes({ etapes }: { etapes: [string, string][] }) {
  return (
    <ol className="flex flex-col gap-2">
      {etapes.map(([titre, detail], index) => (
        <li key={titre} className="flex gap-3 rounded-2xl bg-creme p-3">
          <span className="font-display text-xl text-terracotta-800">{index + 1}.</span>
          <span>
            <span className="font-semibold text-encre">{titre}</span> — {detail}
          </span>
        </li>
      ))}
    </ol>
  );
}

export default function AidePage() {
  return (
    <main className="mx-auto flex w-full max-w-2xl flex-col gap-5 px-6 pb-16">
      <header className="flex flex-col gap-2">
        <h1 className="font-display text-3xl sm:text-4xl">Comment ça marche&nbsp;?</h1>
        <p className="text-neutre-700">
          Le troc, c&apos;est simple : tes objets valent des objets. Voici tout ce qu&apos;il
          faut savoir pour échanger sereinement.
        </p>
      </header>

      <nav
        aria-label="Sommaire"
        className="flex flex-wrap gap-2 rounded-[32px] bg-creme p-4 text-xs"
      >
        {SOMMAIRE.map(([id, label]) => (
          <a
            key={id}
            href={`#${id}`}
            className="rounded-full bg-sable px-3 py-1.5 font-semibold text-encre transition-colors hover:bg-terracotta-100"
          >
            {label}
          </a>
        ))}
      </nav>

      <Section id="compte" titre="Créer son compte" emoji="👋">
        <p>
          Un pseudo, un e-mail, un mot de passe et ton code postal — il sert uniquement à te
          montrer ce qui se troque près de chez toi. Les autres ne voient jamais que ta
          commune, jamais ton adresse. Clique le lien reçu par e-mail pour pouvoir publier.
        </p>
      </Section>

      <Section id="publier" titre="Publier un objet" emoji="📸">
        <p>
          Deux minutes suffisent : 1 à 8 photos, un titre, une catégorie, l&apos;état, une
          description et une <strong className="text-encre">valeur indicative</strong> en
          euros. Ce n&apos;est pas un prix de vente : c&apos;est un ordre de grandeur qui aide
          à composer des échanges équilibrés.
        </p>
        <p>
          Tu choisis le mode de remise (main propre, envoi en point relais, ou les deux) et si
          tu acceptes une <strong className="text-encre">soulte</strong> — un complément en
          euros quand les valeurs ne s&apos;équilibrent pas tout à fait. Ton{" "}
          <Link href="/dressing" className="underline">
            dressing
          </Link>{" "}
          rassemble tous tes objets : disponibles, réservés, troqués ou masqués.
        </p>
      </Section>

      <Section id="trouver" titre="Trouver son bonheur" emoji="🔎">
        <p>
          Le fil d&apos;accueil montre les objets les plus proches et les plus récents. La
          recherche tolère les fautes de frappe et se filtre par catégorie, état, distance,
          mode de remise et soulte.
        </p>
        <p>
          Le cœur ❤️ met un objet en favori : tu seras prévenu s&apos;il est réservé — et
          s&apos;il redevient disponible. Et dans ton profil, «&nbsp;Ce que je cherche&nbsp;»
          enregistre jusqu&apos;à 3 recherches : les objets correspondants remontent
          directement sur ton accueil.
        </p>
      </Section>

      <Section id="proposer" titre="Proposer un troc" emoji="🔁">
        <p>
          Depuis la page d&apos;un objet : compose ta proposition — un ou plusieurs de tes
          objets contre un ou plusieurs des siens, avec au besoin une soulte (plafonnée à
          30&nbsp;% du panier le moins cher), et un petit mot.
        </p>
        <p>
          L&apos;autre partie peut accepter, refuser ou{" "}
          <strong className="text-encre">contre-proposer</strong>. Une conversation en temps
          réel accompagne chaque proposition — les coordonnées (téléphone, e-mail) sont
          automatiquement masquées tant que le troc n&apos;est pas accepté, pour la sécurité
          de tout le monde. Sans réponse, une proposition expire après 7&nbsp;jours.
        </p>
      </Section>

      <Section id="echanger" titre="L'échange" emoji="🤝">
        <p>
          <strong className="text-encre">En main propre</strong> — chacun reçoit un code à
          6&nbsp;chiffres. Au rendez-vous, vous échangez vos codes : le troc est finalisé
          quand les deux sont saisis. S&apos;il y a une soulte, elle est bloquée à
          l&apos;avance sur la carte et n&apos;est débitée que{" "}
          <strong className="text-encre">48&nbsp;h après la remise</strong> — le temps de
          signaler un problème découvert après coup.
        </p>
        <p>
          <strong className="text-encre">Par envoi</strong> — chacun expédie son objet en
          point relais&nbsp;:
        </p>
        <Etapes
          etapes={[
            ["Prépare ton envoi", "choisis le format de ton colis (S 4,50 € · M 6,90 € · L 9,90 €, jamais de pesée) et le relais où tu veux recevoir."],
            ["Bloque ton règlement", "transport + 2 € de service + ta part de soulte éventuelle, en une seule fois. Rien n'est débité à ce stade."],
            ["Dépose ton colis", "dès que les deux ont payé, ton code de dépôt apparaît. Tu as 5 jours (on te rappelle à J+2 et J+4). Si personne ne dépose, tout est annulé et personne n'est débité."],
            ["Récupère et confirme", "quand ton colis arrive, va le chercher et confirme que tout est OK — ou signale un problème. Sans nouvelle, la réception se confirme seule 72 h après le retrait."],
          ]}
        />
        <p>
          Quand les deux colis sont confirmés, le troc est finalisé et les règlements débités.
          L&apos;argent ne circule jamais avant que chacun ait ce qu&apos;il attend.
        </p>
      </Section>

      <Section id="evaluer" titre="Les évaluations" emoji="⭐">
        <p>
          Après chaque troc finalisé, note l&apos;autre partie (1 à 5&nbsp;étoiles +
          commentaire). Ta note reste secrète tant que l&apos;autre n&apos;a pas donné la
          sienne — publication simultanée, pour que personne ne note sous le coup de la
          vengeance. Le noté peut répondre publiquement une fois. Les profils affichent la
          moyenne, le nombre de trocs et le délai d&apos;expédition moyen.
        </p>
      </Section>

      <Section id="probleme" titre="Si ça se passe mal" emoji="⚖️">
        <p>
          <strong className="text-encre">Ouvre un dossier</strong> depuis la page du troc :
          colis non conforme, abîmé ou manquant (avant confirmation), rendez-vous fantôme (à
          partir du 3ᵉ jour), ou défaut découvert après une remise en main propre (sous
          48&nbsp;h). Décris le problème, joins jusqu&apos;à 5 photos — elles restent
          privées. L&apos;autre partie a 72&nbsp;h pour donner sa version, puis
          l&apos;équipe tranche sous 7&nbsp;jours. Pendant l&apos;examen, l&apos;argent
          reste bloqué : personne n&apos;est débité tant que rien n&apos;est tranché.
        </p>
        <p>
          Tu peux aussi <strong className="text-encre">signaler</strong> un objet ou un
          profil (bouton dédié sur chaque page) et{" "}
          <strong className="text-encre">bloquer</strong> un troqueur : plus de propositions
          ni de messages dans les deux sens, en toute discrétion. Les fraudeurs récidivistes
          sont automatiquement avertis, restreints puis bannis.
        </p>
      </Section>

      <Section id="notifications" titre="Les notifications" emoji="🔔">
        <p>
          La cloche regroupe tout : propositions, paiements, colis, évaluations, litiges,
          favoris. Dans{" "}
          <Link href="/reglages/notifications" className="underline">
            Réglages → Notifications
          </Link>
          , choisis ce qui arrive aussi par e-mail. Ce qui touche à l&apos;argent, aux colis
          et aux litiges est toujours envoyé — c&apos;est la sécurité de tes trocs.
        </p>
      </Section>

      <Section id="donnees" titre="Tes données" emoji="🔐">
        <p>
          Depuis ton profil : télécharge toutes tes données (JSON) ou supprime ton compte à
          tout moment (ton profil est anonymisé, tes objets retirés). Pas de pub, pas de
          revente de données, pas de traceur tiers — le détail dans notre{" "}
          <Link href="/confidentialite" className="underline">
            politique de confidentialité
          </Link>
          , les règles du jeu dans les{" "}
          <Link href="/cgu" className="underline">
            CGU
          </Link>
          .
        </p>
      </Section>

      <section className="flex flex-wrap items-center justify-between gap-3 rounded-[32px] bg-sauge-100 p-6">
        <p className="font-display text-lg text-sauge-800">Prêt·e à tenter ton premier troc ?</p>
        <Link
          href="/publier"
          className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
        >
          Publier un objet
        </Link>
      </section>
    </main>
  );
}
