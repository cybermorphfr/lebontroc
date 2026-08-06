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
    <main className="mx-auto flex w-full max-w-3xl flex-col gap-4 p-6 font-mono text-sm">
      <h1 className="text-xl font-bold">Litiges ({details.length})</h1>

      <form action={liftSanctions} className="flex items-center gap-2 rounded border p-3">
        <input name="pseudo" placeholder="pseudo" className="border px-2 py-1" />
        <button type="submit" className="border px-3 py-1 hover:bg-black/5">
          Lever les sanctions
        </button>
      </form>

      {details.map((d) => (
        <details key={d.id} className="rounded border p-3" open={d.status !== "tranche"}>
          <summary className="cursor-pointer">
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
                      className="h-24 w-24 border object-cover"
                    />
                  </a>
                ))}
              </div>
            ) : null}
            {d.status !== "tranche" ? (
              <form action={resolveDispute} className="flex flex-wrap items-center gap-2 border-t pt-2">
                <input type="hidden" name="id" value={d.id} />
                <select name="outcome" className="border px-2 py-1" required>
                  <option value="capture">capture (troc validé, débits)</option>
                  <option value="liberation">libération (annulation, zéro débit)</option>
                  <option value="rejet">rejet (classé sans suite)</option>
                </select>
                <input
                  name="penalized"
                  placeholder="pseudo en tort (option)"
                  className="border px-2 py-1"
                />
                <input name="note" placeholder="note interne" className="border px-2 py-1" />
                <button type="submit" className="border bg-black px-3 py-1 text-white">
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
