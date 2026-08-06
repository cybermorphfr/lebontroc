import type { TradeDetailResponse } from "@lebontroc/api-client";

/**
 * Où en est le troc, en une ligne. Jusqu'ici l'avancement se devinait :
 * il fallait comprendre quel panneau s'affichait et lire les phrases
 * dispersées. Ici on nomme l'étape en cours, on dit qui doit agir, et on
 * montre le chemin restant.
 */

type Etape = {
  cle: string;
  titre: string;
  /** Ce qui se passe à cette étape, du point de vue du lecteur. */
  detail: string;
};

const ETAPES_ENVOI: Etape[] = [
  { cle: "accord", titre: "Accord", detail: "La proposition est acceptée." },
  { cle: "paiement", titre: "Règlements", detail: "Chacun sécurise transport et soulte." },
  { cle: "depot", titre: "Dépôts", detail: "Chacun dépose son colis en point relais." },
  { cle: "livraison", titre: "Retraits", detail: "Chacun récupère le colis de l'autre." },
  { cle: "confirmation", titre: "Confirmations", detail: "Chacun confirme avoir bien reçu." },
];

const ETAPES_MAIN_PROPRE: Etape[] = [
  { cle: "accord", titre: "Accord", detail: "La proposition est acceptée." },
  { cle: "paiement", titre: "Règlement", detail: "La soulte est sécurisée." },
  { cle: "rendez_vous", titre: "Rendez-vous", detail: "Vous convenez d'un lieu et d'une heure." },
  { cle: "remise", titre: "Remise", detail: "Chacun saisit le code de l'autre." },
];

type Etat = {
  /** Index de l'étape en cours dans la liste. */
  index: number;
  /** La phrase qui répond à « et maintenant ? ». */
  attente: string;
  /** Le lecteur est-il celui qui doit agir ? */
  aMoiDAgir: boolean;
  termine: boolean;
  arrete: boolean;
};

function etatEnvoi(trade: TradeDetailResponse): Etat {
  const mien = trade.shipments.find((s) => s.i_am_sender);
  const entrant = trade.shipments.find((s) => !s.i_am_sender);
  const paye = trade.payment?.status === "sequestre" || trade.payment?.status === "capture";
  const autrePaye =
    trade.other_payment_status === "sequestre" || trade.other_payment_status === "capture";

  if (trade.status === "attente_paiement") {
    if (!paye) {
      return {
        index: 1,
        attente: "À toi de sécuriser ton règlement (transport et soulte éventuelle).",
        aMoiDAgir: true,
        termine: false,
        arrete: false,
      };
    }
    return {
      index: 1,
      attente: `Ton règlement est sécurisé. On attend celui de ${autrePaye ? "personne" : "l'autre partie"} pour éditer les étiquettes.`,
      aMoiDAgir: false,
      termine: false,
      arrete: false,
    };
  }

  if (mien && mien.status !== "confirme" && ["preparation", "etiquette"].includes(mien.status)) {
    return {
      index: 2,
      attente: "À toi de déposer ton colis en point relais.",
      aMoiDAgir: true,
      termine: false,
      arrete: false,
    };
  }
  if (entrant && ["depose", "transit"].includes(entrant.status)) {
    return {
      index: 3,
      attente: "Ton colis est parti ; celui de l'autre est en route.",
      aMoiDAgir: false,
      termine: false,
      arrete: false,
    };
  }
  if (entrant?.status === "arrive") {
    return {
      index: 3,
      attente: "Ton colis t'attend au point relais — à toi de le récupérer.",
      aMoiDAgir: true,
      termine: false,
      arrete: false,
    };
  }
  if (entrant?.status === "retire") {
    return {
      index: 4,
      attente: "Tu l'as récupéré : confirme que tout est conforme (ou signale un problème).",
      aMoiDAgir: true,
      termine: false,
      arrete: false,
    };
  }
  if (entrant?.status === "confirme" && mien?.status !== "confirme") {
    return {
      index: 4,
      attente: "Tu as confirmé. On attend la confirmation de l'autre partie.",
      aMoiDAgir: false,
      termine: false,
      arrete: false,
    };
  }
  return {
    index: 2,
    attente: "Les colis sont en chemin.",
    aMoiDAgir: false,
    termine: false,
    arrete: false,
  };
}

function etatMainPropre(trade: TradeDetailResponse): Etat {
  const paye = trade.payment?.status === "sequestre" || trade.payment?.status === "capture";
  if (trade.status === "attente_paiement") {
    const jePaie = trade.payment?.i_am_payer ?? false;
    return {
      index: 1,
      attente: jePaie
        ? "À toi de sécuriser la soulte pour débloquer les codes de remise."
        : "On attend que l'autre partie sécurise la soulte.",
      aMoiDAgir: jePaie,
      termine: false,
      arrete: false,
    };
  }
  if (trade.i_confirmed && !trade.other_confirmed) {
    return {
      index: 3,
      attente: "Tu as saisi son code. On attend qu'elle saisisse le tien.",
      aMoiDAgir: false,
      termine: false,
      arrete: false,
    };
  }
  if (!trade.i_confirmed && trade.other_confirmed) {
    return {
      index: 3,
      attente: "L'autre partie a saisi ton code — à toi de saisir le sien pour finaliser.",
      aMoiDAgir: true,
      termine: false,
      arrete: false,
    };
  }
  return {
    index: 2,
    attente: paye
      ? "Convenez d'un lieu et d'une heure, puis échangez vos codes sur place."
      : "Convenez d'un lieu et d'une heure de rendez-vous.",
    aMoiDAgir: true,
    termine: false,
    arrete: false,
  };
}

export function Avancement({ trade }: { trade: TradeDetailResponse }) {
  const envoi = trade.delivery_mode === "envoi";
  const etapes = envoi ? ETAPES_ENVOI : ETAPES_MAIN_PROPRE;

  let etat: Etat;
  if (trade.status === "finalise") {
    etat = {
      index: etapes.length - 1,
      attente: "Troc terminé — pensez à vous évaluer.",
      aMoiDAgir: false,
      termine: true,
      arrete: false,
    };
  } else if (trade.status === "annule") {
    etat = {
      index: 0,
      attente: "Troc annulé. Les objets sont de nouveau disponibles.",
      aMoiDAgir: false,
      termine: false,
      arrete: true,
    };
  } else if (trade.status === "litige_gele" || trade.dispute) {
    etat = {
      index: etapes.length - 1,
      attente: "Un litige est ouvert : l'équipe examine le dossier avant de trancher.",
      aMoiDAgir: false,
      termine: false,
      arrete: true,
    };
  } else if (envoi) {
    etat = etatEnvoi(trade);
  } else {
    etat = etatMainPropre(trade);
  }

  const ton = etat.arrete
    ? "bg-terracotta-100 text-terracotta-800"
    : etat.termine
      ? "bg-sauge-100 text-sauge-800"
      : etat.aMoiDAgir
        ? "bg-terracotta-100 text-terracotta-800"
        : "bg-sable text-encre";

  return (
    <section
      aria-label="Avancement du troc"
      className="flex flex-col gap-3 rounded-[28px] bg-sable p-4 shadow-sm"
    >
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="font-display text-base">Où en est ce troc ?</h2>
        <span
          data-testid="avancement-attente"
          className={`rounded-full px-3 py-1 text-xs font-semibold ${ton}`}
        >
          {etat.termine
            ? "✓ Terminé"
            : etat.arrete
              ? "En pause"
              : etat.aMoiDAgir
                ? "C'est à toi"
                : "En attente de l'autre"}
        </span>
      </div>

      {/* Le chemin : chaque étape franchie, celle en cours, celles à venir. */}
      <ol className="flex flex-wrap items-stretch gap-1.5">
        {etapes.map((etape, i) => {
          const franchie = i < etat.index || etat.termine;
          const courante = i === etat.index && !etat.termine;
          return (
            <li
              key={etape.cle}
              className={`flex min-w-24 flex-1 flex-col gap-0.5 rounded-2xl px-3 py-2 ${
                courante
                  ? "bg-creme ring-2 ring-terracotta-500"
                  : franchie
                    ? "bg-sauge-100"
                    : "bg-creme/60"
              }`}
            >
              <span className="flex items-center gap-1 text-xs font-semibold">
                {franchie ? <span className="text-sauge-800">✓</span> : null}
                <span className={franchie ? "text-sauge-800" : ""}>{etape.titre}</span>
              </span>
              <span className="text-[11px] leading-tight text-neutre-700">{etape.detail}</span>
            </li>
          );
        })}
      </ol>

      <p className="text-sm">{etat.attente}</p>
    </section>
  );
}
