"use client";

import { useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import type { ItemResponse } from "@lebontroc/api-client";

import { MessageSuggestions } from "@/components/MessageSuggestions";
import { Segmented } from "@/components/ui/Segmented";
import { Textarea } from "@/components/ui/Textarea";
import { apiFetch, apiError } from "@/lib/client-api";
import { ECHANGE, euros } from "@/lib/format";
import { suggestionsProposition } from "@/lib/suggestions";

/** Le composeur « ça contre ça » — l'écran signature de Lebontroc. */
export function ProposalComposer({
  mine,
  theirs,
  recipientPseudo,
  preselectedRequested,
  preselectedOffered = [],
  counterOf,
}: {
  mine: ItemResponse[];
  theirs: ItemResponse[];
  recipientPseudo: string;
  preselectedRequested: string[];
  preselectedOffered?: string[];
  /** Id de la proposition à remplacer (mode contre-proposition). */
  counterOf?: string;
}) {
  const router = useRouter();
  const [offered, setOffered] = useState<Set<string>>(new Set(preselectedOffered));
  const [requested, setRequested] = useState<Set<string>>(new Set(preselectedRequested));
  const [cashDirection, setCashDirection] = useState("aucune");
  const [cashEuros, setCashEuros] = useState(0);
  const [message, setMessage] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void apiFetch("/analytics/track", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "proposal_composer_opened" }),
    });
  }, []);

  // Plafond domaine : 50 % de la valeur du meilleur objet sélectionné.
  const plafondEuros = useMemo(() => {
    const values = [...mine, ...theirs]
      .filter((i) => offered.has(i.id) || requested.has(i.id))
      .map((i) => i.value_cents);
    return Math.floor(Math.max(0, ...values) / 2 / 100);
  }, [mine, theirs, offered, requested]);

  useEffect(() => {
    // Le curseur est bloqué au plafond quand la sélection change (Gherkin).
    setCashEuros((current) => Math.min(current, plafondEuros));
  }, [plafondEuros]);

  function toggle(set: Set<string>, id: string, update: (next: Set<string>) => void) {
    const next = new Set(set);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    update(next);
  }

  async function send() {
    setSending(true);
    setError(null);
    try {
      const endpoint = counterOf ? `/proposals/${counterOf}/counter` : "/proposals";
      const response = await apiFetch(endpoint, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          offered_item_ids: [...offered],
          requested_item_ids: [...requested],
          cash_cents: cashDirection === "aucune" ? 0 : cashEuros * 100,
          cash_direction: cashDirection === "aucune" ? null : cashDirection,
          message: message.trim() || null,
        }),
      });
      if (!response.ok) {
        setError((await apiError(response)).message);
        return;
      }
      const proposal = (await response.json()) as { id: string };
      router.push(`/trocs/${proposal.id}`);
    } finally {
      setSending(false);
    }
  }

  const ready = offered.size > 0 && requested.size > 0;

  // Écart de valeur AVANT soulte : positif = je reçois plus que je ne donne.
  const ecartCents = useMemo(() => {
    const somme = (liste: ItemResponse[], selection: Set<string>) =>
      liste.filter((i) => selection.has(i.id)).reduce((t, i) => t + i.value_cents, 0);
    return somme(theirs, requested) - somme(mine, offered);
  }, [mine, theirs, offered, requested]);

  /// Propose la soulte qui recolle l'écart, dans la limite du plafond.
  function equilibrer() {
    const montant = Math.min(Math.round(Math.abs(ecartCents) / 100), plafondEuros);
    setCashDirection(ecartCents > 0 ? "du_proposant" : "du_destinataire");
    setCashEuros(montant);
  }
  /// Ce qui manque pour pouvoir envoyer — affiché sous le bouton grisé.
  const raisonBlocage = useMemo(() => {
    if (ready) return null;
    if (mine.length === 0) {
      return "Tu n'as pas encore d'objet disponible à échanger : publie-en un et reviens ici.";
    }
    if (offered.size === 0 && requested.size === 0) {
      return `Choisis au moins un de tes objets à donner et un objet de ${recipientPseudo}.`;
    }
    if (offered.size === 0) {
      return "Il manque ce que TU donnes : choisis au moins un de tes objets ci-dessus.";
    }
    return `Il manque ce que tu reçois : choisis au moins un objet de ${recipientPseudo}.`;
  }, [ready, mine.length, offered.size, requested.size, recipientPseudo]);
  const soldeCents =
    [...mine.filter((i) => offered.has(i.id))].reduce((s, i) => s + i.value_cents, 0) +
    (cashDirection === "du_proposant" ? cashEuros * 100 : 0) -
    [...theirs.filter((i) => requested.has(i.id))].reduce((s, i) => s + i.value_cents, 0) -
    (cashDirection === "du_destinataire" ? cashEuros * 100 : 0);

  return (
    <div className="flex flex-col gap-6">
      <div className="grid gap-4 md:grid-cols-2">
        <SelectColumn
          title="Tu donnes"
          items={mine}
          selected={offered}
          onToggle={(id) => toggle(offered, id, setOffered)}
        />
        <SelectColumn
          title="Tu reçois"
          items={theirs}
          selected={requested}
          onToggle={(id) => toggle(requested, id, setRequested)}
        />
      </div>

      <section className="flex flex-col gap-3 rounded-[32px] bg-sable p-5 shadow-sm">
        <h2 className="font-display text-lg">Une soulte pour équilibrer ?</h2>
        <p className="-mt-1 text-xs text-neutre-700">
          Si l&apos;échange penche d&apos;un côté, ajoute des euros — ou demande à{" "}
          {recipientPseudo} d&apos;en ajouter.
        </p>
        <Segmented
          options={[
            { value: "aucune", label: "Pas de soulte" },
            { value: "du_proposant", label: "J'ajoute des euros" },
            { value: "du_destinataire", label: "J'en demande" },
          ]}
          value={cashDirection}
          onChange={setCashDirection}
        />
        {cashDirection !== "aucune" ? (
          plafondEuros > 0 ? (
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center gap-3">
                <input
                  type="range"
                  aria-label="Montant de la soulte"
                  min={0}
                  max={plafondEuros}
                  step={1}
                  value={cashEuros}
                  onChange={(e) => setCashEuros(Number(e.target.value))}
                  className="flex-1 accent-[#c67139]"
                />
                <span className="w-16 text-right font-display text-lg">{cashEuros} €</span>
              </div>
              <p className="text-xs text-neutre-700">
                Plafond : {plafondEuros} € — la soulte ne peut pas dépasser 50 % de la valeur du
                meilleur objet de l&apos;échange.
              </p>
              <p className="rounded-2xl bg-terracotta-100/60 px-3 py-2 text-xs text-terracotta-800">
                La soulte est sécurisée : bloquée sur la carte à l&apos;acceptation, elle
                n&apos;est transférée qu&apos;une fois la remise confirmée par vos deux codes.
              </p>
            </div>
          ) : (
            <p className="text-xs text-neutre-700">Choisis d&apos;abord des objets.</p>
          )
        ) : null}
      </section>

      <div className="flex flex-col gap-2">
        <Textarea
          id="message"
          label="Un mot pour accompagner (optionnel)"
          placeholder="Salut ! Ton vélo m'irait bien — ma console t'intéresse ?"
          value={message}
          onChange={(e) => setMessage(e.target.value.slice(0, 500))}
        />
        {message.trim() === "" ? (
          <MessageSuggestions
            suggestions={suggestionsProposition(
              theirs.find((i) => requested.has(i.id))?.title ?? "objet",
            )}
            onPick={(suggestion) => setMessage(suggestion)}
            label="Suggestions de mot d'accompagnement"
          />
        ) : null}
      </div>

      <section className="flex flex-col gap-2 rounded-[32px] bg-sable p-5 shadow-sm">
        <h2 className="font-display text-lg">Récap</h2>
        {ready ? <Solde ecartCents={ecartCents} plafondEuros={plafondEuros} onEquilibrer={equilibrer} /> : null}
        <div className="grid grid-cols-2 gap-3 text-sm">
          <RecapColumn
            title="Tu donnes"
            ton="donne"
            items={mine.filter((i) => offered.has(i.id))}
            extra={cashDirection === "du_proposant" && cashEuros > 0 ? `+ ${cashEuros} €` : null}
            extraCents={cashDirection === "du_proposant" ? cashEuros * 100 : 0}
          />
          <RecapColumn
            title="Tu reçois"
            ton="recoit"
            items={theirs.filter((i) => requested.has(i.id))}
            extra={
              cashDirection === "du_destinataire" && cashEuros > 0 ? `+ ${cashEuros} €` : null
            }
            extraCents={cashDirection === "du_destinataire" ? cashEuros * 100 : 0}
          />
        </div>
        {ready ? (
          <p className="text-xs text-neutre-700">
            {soldeCents === 0
              ? "Échange équilibré, au centime près — beau troc."
              : soldeCents > 0
                ? `Tu donnes environ ${Math.round(soldeCents / 100)} € de plus — à toi de voir si ça vaut le coup.`
                : `Tu reçois environ ${Math.round(-soldeCents / 100)} € de plus — belle affaire.`}
          </p>
        ) : null}
      </section>

      {error ? (
        <p className="rounded-full bg-terracotta-100 px-4 py-2 text-sm text-terracotta-800">
          {error}
        </p>
      ) : null}

      <div className="flex flex-col gap-2">
        <button
          onClick={send}
          disabled={!ready || sending}
          title={raisonBlocage ?? undefined}
          aria-describedby={raisonBlocage ? "raison-blocage" : undefined}
          className="flex min-h-12 cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-6 font-display text-base text-creme transition-colors hover:bg-terracotta-600 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {sending
            ? "Envoi…"
            : counterOf
              ? `Envoyer ma contre-proposition à ${recipientPseudo}`
              : `Envoyer ma proposition à ${recipientPseudo}`}
        </button>
        {raisonBlocage ? (
          <p
            id="raison-blocage"
            role="status"
            className="flex items-start gap-2 rounded-3xl bg-terracotta-100/70 px-4 py-2.5 text-sm text-terracotta-800"
          >
            <span aria-hidden>💡</span>
            <span>
              {raisonBlocage}
              {mine.length === 0 ? (
                <>
                  {" "}
                  <a href="/publier" className="font-semibold underline">
                    Publier un objet
                  </a>
                </>
              ) : null}
            </span>
          </p>
        ) : null}
      </div>
    </div>
  );
}

function SelectColumn({
  title,
  items,
  selected,
  onToggle,
}: {
  title: string;
  items: ItemResponse[];
  selected: Set<string>;
  onToggle: (id: string) => void;
}) {
  return (
    <section className="flex flex-col gap-3 rounded-[32px] bg-sable p-5 shadow-sm">
      <h2 className="font-display text-lg">{title}</h2>
      {items.length === 0 ? (
        <p className="text-sm text-neutre-700">Rien de disponible ici.</p>
      ) : (
        <ul className="grid grid-cols-2 gap-2.5">
          {items.map((item) => {
            const checked = selected.has(item.id);
            return (
              <li key={item.id}>
                <button
                  onClick={() => onToggle(item.id)}
                  aria-pressed={checked}
                  aria-label={`${checked ? "Retirer" : "Choisir"} ${item.title}`}
                  className={`flex w-full flex-col overflow-hidden rounded-2xl border-2 text-left transition-colors ${
                    checked ? "border-terracotta-500 bg-terracotta-100/40" : "border-transparent bg-creme"
                  }`}
                >
                  <div className="relative aspect-square w-full bg-neutre-100">
                    {item.photos[0] ? (
                      // eslint-disable-next-line @next/next/no-img-element
                      <img
                        src={item.photos[0].url}
                        alt=""
                        className="size-full object-cover"
                      />
                    ) : null}
                    {checked ? (
                      <span className="absolute right-1.5 top-1.5 flex size-6 items-center justify-center rounded-full bg-terracotta-500 text-sm text-creme">
                        ✓
                      </span>
                    ) : null}
                  </div>
                  <div className="flex flex-col gap-0.5 p-2">
                    <span className="truncate text-xs font-semibold">{item.title}</span>
                    <span className="text-[11px] text-neutre-700">
                      ~{Math.round(item.value_cents / 100)} €
                    </span>
                  </div>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

function RecapColumn({
  title,
  items,
  extra,
  extraCents = 0,
  ton,
}: {
  title: string;
  items: ItemResponse[];
  extra: string | null;
  /// Soulte en centimes, pour le total.
  extraCents?: number;
  /// Convention de toute l'application : ce qui part / ce qui arrive.
  ton: "donne" | "recoit";
}) {
  const style = ECHANGE[ton];
  const total = items.reduce((somme, item) => somme + item.value_cents, 0) + extraCents;
  return (
    <div className={`flex flex-col gap-1 rounded-2xl p-3 ${style.fond}`}>
      <span className={`text-xs font-semibold ${style.texte}`}>{title}</span>
      {items.length === 0 ? <span className="text-neutre-700">—</span> : null}
      {items.map((item) => (
        <span key={item.id} className="truncate">
          {item.title} <span className="text-neutre-700">~{euros(item.value_cents)}</span>
        </span>
      ))}
      {extra ? <span className={`font-semibold ${style.texte}`}>{extra}</span> : null}
      <span className={`mt-1 border-t border-encre/10 pt-1 text-sm font-semibold ${style.texte}`}>
        Total ~{euros(total)}
      </span>
    </div>
  );
}

/**
 * Le déséquilibre de l'échange, chiffré — avec le geste qui le corrige :
 * ajouter des euros, ou en demander.
 */
function Solde({
  ecartCents,
  plafondEuros,
  onEquilibrer,
}: {
  ecartCents: number;
  plafondEuros: number;
  onEquilibrer: () => void;
}) {
  if (ecartCents === 0) {
    return (
      <p className="rounded-2xl bg-sauge-100 px-3 py-2 text-xs text-sauge-800">
        ⚖️ Les valeurs s&apos;équilibrent — pas besoin de soulte.
      </p>
    );
  }
  const enMaFaveur = ecartCents > 0;
  const ecartEuros = Math.round(Math.abs(ecartCents) / 100);
  return (
    <div className="flex flex-wrap items-center justify-between gap-2 rounded-2xl bg-creme px-3 py-2 text-xs">
      <span className="text-neutre-700">
        ⚖️ L&apos;échange penche de <strong>{ecartEuros} €</strong>{" "}
        {enMaFaveur ? "en ta faveur" : "en ta défaveur"}.
      </span>
      {plafondEuros > 0 ? (
        <button
          type="button"
          onClick={onEquilibrer}
          className="cursor-pointer rounded-full border border-terracotta-500 px-3 py-1 font-semibold text-terracotta-800 transition-colors hover:bg-terracotta-100"
        >
          {enMaFaveur ? `Ajouter ${Math.min(ecartEuros, plafondEuros)} €` : `Demander ${Math.min(ecartEuros, plafondEuros)} €`}
        </button>
      ) : null}
    </div>
  );
}
