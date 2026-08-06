import { revalidatePath } from "next/cache";
import { adminFetch, adminPost } from "../adminFetch";

export const dynamic = "force-dynamic";

// Page d'administration brute des litiges (F5.2) — protégée par la basic
// auth Traefik sur /admin ET le token API. L'ergonomie attendra F6.1.


type Summary = {
  id: string;
  trade_id: string;
  reason: string;
  status: string;
  opened_at: string;
  opened_by_pseudo: string | null;
};

type Detail = Summary & {
  trade_status: string;
  delivery_mode: string;
  cash_cents: number;
  description: string;
  response: string | null;
  proposer_pseudo: string;
  recipient_pseudo: string;
  proposer_score: number;
  recipient_score: number;
  photos: { uploader_pseudo: string; url: string }[];
  payments: { payer_pseudo: string; amount_cents: number; status: string }[];
  outcome: string | null;
  admin_note: string | null;
};

async function resolveDispute(formData: FormData) {
  "use server";
  const id = formData.get("id");
  const outcome = formData.get("outcome");
  const penalized = (formData.get("penalized") as string | null)?.trim();
  const note = (formData.get("note") as string | null)?.trim();
  await adminPost(`/admin/disputes/${id}/resolve`, {
      outcome,
      penalized_pseudo: penalized || null,
      note: note || null,
    });
  revalidatePath("/admin/litiges");
}

async function liftSanctions(formData: FormData) {
  "use server";
  const pseudo = (formData.get("pseudo") as string | null)?.trim();
  if (pseudo) {
    await adminPost(`/admin/users/${encodeURIComponent(pseudo)}/lift-sanctions`);
  }
  revalidatePath("/admin/litiges");
}

export default async function AdminLitigesPage() {
  const summaries = (await adminFetch<Summary[]>("/admin/disputes")) ?? [];
  const details = (
    await Promise.all(summaries.map((s) => adminFetch<Detail>(`/admin/disputes/${s.id}`)))
  ).filter((d): d is Detail => d !== null);

  return (
    <main className="flex flex-col gap-4 text-sm">
      <h1 className="font-display text-2xl">Litiges ({details.length})</h1>

      <form action={liftSanctions} className="flex flex-wrap items-center gap-2 rounded-[28px] bg-sable p-4 shadow-sm">
        <input name="pseudo" placeholder="pseudo" className="rounded-full border border-neutre-300 bg-creme px-3.5 py-2 text-sm outline-none focus:border-terracotta-500" />
        <button type="submit" className="cursor-pointer rounded-full border border-neutre-300 px-4 py-2 text-sm hover:bg-encre/7">
          Lever les sanctions
        </button>
      </form>

      {details.map((d) => (
        <details key={d.id} className="rounded-[28px] bg-sable p-5 shadow-sm" open={d.status !== "tranche"}>
          <summary className="cursor-pointer font-display text-base">
            [{d.status}] {d.reason} — {d.proposer_pseudo} ↔ {d.recipient_pseudo} ·{" "}
            {new Date(d.opened_at).toLocaleDateString("fr-FR")}
            {d.outcome ? ` → ${d.outcome}` : ""}
          </summary>
          <div className="mt-2 flex flex-col gap-2">
            <p>
              troc {d.trade_id} · {d.delivery_mode} · statut {d.trade_status} · soulte{" "}
              {(d.cash_cents / 100).toFixed(2)} € · ouvert par {d.opened_by_pseudo ?? "système"}
            </p>
            <p>
              scores : {d.proposer_pseudo} = {d.proposer_score} · {d.recipient_pseudo} ={" "}
              {d.recipient_score}
            </p>
            <p>« {d.description} »</p>
            {d.response ? <p>réponse : « {d.response} »</p> : <p>(pas de réponse)</p>}
            {d.payments.length > 0 ? (
              <p>
                paiements :{" "}
                {d.payments
                  .map(
                    (p) => `${p.payer_pseudo} ${(p.amount_cents / 100).toFixed(2)} € (${p.status})`,
                  )
                  .join(" · ")}
              </p>
            ) : null}
            {d.photos.length > 0 ? (
              <div className="flex flex-wrap gap-2">
                {d.photos.map((photo, index) => (
                  <a key={index} href={photo.url} target="_blank" rel="noreferrer">
                    {/* eslint-disable-next-line @next/next/no-img-element */}
                    <img
                      src={photo.url}
                      alt={`pièce de ${photo.uploader_pseudo}`}
                      className="h-24 w-24 rounded-2xl object-cover"
                    />
                  </a>
                ))}
              </div>
            ) : null}
            {d.status !== "tranche" ? (
              <form action={resolveDispute} className="flex flex-wrap items-center gap-2 border-t border-encre/10 pt-3">
                <input type="hidden" name="id" value={d.id} />
                <select name="outcome" className="rounded-full border border-neutre-300 bg-creme px-3 py-2 text-sm outline-none" required>
                  <option value="capture">capture (troc validé, débits)</option>
                  <option value="liberation">libération (annulation, zéro débit)</option>
                  <option value="rejet">rejet (classé sans suite)</option>
                </select>
                <input
                  name="penalized"
                  placeholder="pseudo en tort (option)"
                  className="rounded-full border border-neutre-300 bg-creme px-3.5 py-2 text-sm outline-none"
                />
                <input name="note" placeholder="note interne" className="rounded-full border border-neutre-300 bg-creme px-3.5 py-2 text-sm outline-none" />
                <button type="submit" className="cursor-pointer rounded-full bg-[#c67139] px-5 py-2 font-display text-sm text-creme hover:bg-terracotta-600">
                  Trancher
                </button>
              </form>
            ) : (
              <p>
                tranché → {d.outcome}
                {d.admin_note ? ` · note : ${d.admin_note}` : ""}
              </p>
            )}
          </div>
        </details>
      ))}
      {details.length === 0 ? <p>Aucun dossier. Tout va bien.</p> : null}
    </main>
  );
}
