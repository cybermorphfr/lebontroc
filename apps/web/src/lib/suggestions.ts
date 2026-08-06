/**
 * Suggestions de messages proposées sous le champ de saisie (F-UX) : le
 * clic PRÉ-REMPLIT le champ, jamais d'envoi direct — on garde la main.
 * Aucune suggestion ne contient de coordonnées : la messagerie les
 * masquerait, et l'échange doit rester sur la plateforme.
 */

/** À la création d'une proposition de troc. */
export function suggestionsProposition(objet: string): string[] {
  const titre = objet.length > 24 ? `${objet.slice(0, 24)}…` : objet;
  return [
    `Salut ! Ton « ${titre} » m'intéresse beaucoup 🙂`,
    "Dis-moi si tu veux d'autres photos du mien.",
    "Je peux te l'apporter cette semaine si ça te va.",
    "Mon objet est très peu servi, il est comme neuf.",
    "Si l'échange ne te convient pas, dis-moi ce que tu cherches !",
  ];
}

/** Dans la conversation, selon l'avancement du troc. */
export function suggestionsConversation(contexte: {
  proposalStatus: string;
  tradeStatus?: string | null;
  deliveryMode?: string | null;
  isProposer: boolean;
  aucunMessage: boolean;
}): string[] {
  const { proposalStatus, tradeStatus, deliveryMode, isProposer, aucunMessage } = contexte;

  if (tradeStatus === "finalise") {
    return ["Merci pour cet échange 🙏", "C'était un plaisir !", "Au plaisir de retroquer 🙂"];
  }

  if (proposalStatus === "acceptee") {
    return deliveryMode === "envoi"
      ? [
          "Je dépose mon colis demain 📦",
          "C'est déposé, il est en route !",
          "Bien reçu, merci beaucoup !",
          "Tu l'as déposé de ton côté ?",
        ]
      : [
          "Quand es-tu disponible pour la remise ?",
          "On se retrouve où ?",
          "Je peux passer ce week-end.",
          "Ça marche pour moi 👍",
        ];
  }

  if (aucunMessage) {
    return isProposer
      ? [
          "Salut ! Est-ce que mon objet t'intéresse ?",
          "Dis-moi si tu veux d'autres photos.",
          "Je peux ajuster la proposition si besoin.",
        ]
      : [
          "Ça m'intéresse !",
          "Il est toujours disponible ?",
          "Tu aurais d'autres photos ?",
          "Et si je te proposais un autre objet ?",
        ];
  }

  return [
    "Il est toujours disponible ?",
    "Tu aurais d'autres photos ?",
    "Ça te va comme échange ?",
    "Merci !",
  ];
}
