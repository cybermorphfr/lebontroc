"use client";

import { useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import type { ItemResponse } from "@lebontroc/api-client";

import { Segmented } from "@/components/ui/Segmented";
import { Textarea } from "@/components/ui/Textarea";
import { apiFetch, apiError } from "@/lib/client-api";

/** Le composeur « ça contre ça » — l'écran signature de Lebontroc. */
export function ProposalComposer({
  mine,
  theirs,
  recipientPseudo,
  preselectedId,
}: {
  mine: ItemResponse[];
  theirs: ItemResponse[];
  recipientPseudo: string;
  preselectedId: string;
}) {
  const router = useRouter();
  const [offered, setOffered] = useState<Set<string>>(new Set());
  const [requested, setRequested] = useState<Set<string>>(new Set([preselectedId]));
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
      const response = await apiFetch("/proposals", {
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
        <Segmented
          options={[
            { value: "aucune", label: "Pas de soulte" },
            { value: "du_proposant", label: "J'ajoute des euros" },
            { value: "du_destinataire", label: `${recipientPseudo} en ajoute` },
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
            </div>
          ) : (
            <p className="text-xs text-neutre-700">Choisis d&apos;abord des objets.</p>
          )
        ) : null}
      </section>

      <Textarea
        id="message"
        label="Un mot pour accompagner (optionnel)"
        placeholder="Salut ! Ton vélo m'irait bien — ma console t'intéresse ?"
        value={message}
        onChange={(e) => setMessage(e.target.value.slice(0, 500))}
      />

      <section className="flex flex-col gap-2 rounded-[32px] bg-sable p-5 shadow-sm">
        <h2 className="font-display text-lg">Récap</h2>
        <div className="grid grid-cols-2 gap-3 text-sm">
          <RecapColumn
            title="Tu donnes"
            items={mine.filter((i) => offered.has(i.id))}
            extra={cashDirection === "du_proposant" && cashEuros > 0 ? `+ ${cashEuros} €` : null}
          />
          <RecapColumn
            title="Tu reçois"
            items={theirs.filter((i) => requested.has(i.id))}
            extra={
              cashDirection === "du_destinataire" && cashEuros > 0 ? `+ ${cashEuros} €` : null
            }
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

      <button
        onClick={send}
        disabled={!ready || sending}
        className="flex min-h-12 cursor-pointer items-center justify-center rounded-full bg-[#c67139] px-6 font-display text-base text-creme transition-colors hover:bg-terracotta-600 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {sending ? "Envoi…" : `Envoyer ma proposition à ${recipientPseudo}`}
      </button>
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
}: {
  title: string;
  items: ItemResponse[];
  extra: string | null;
}) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-xs font-semibold text-neutre-700">{title}</span>
      {items.length === 0 ? <span className="text-neutre-700">—</span> : null}
      {items.map((item) => (
        <span key={item.id} className="truncate">
          {item.title}{" "}
          <span className="text-neutre-700">~{Math.round(item.value_cents / 100)} €</span>
        </span>
      ))}
      {extra ? <span className="font-semibold text-terracotta-700">{extra}</span> : null}
    </div>
  );
}
