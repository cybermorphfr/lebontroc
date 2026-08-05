import { revalidatePath } from "next/cache";

export const dynamic = "force-dynamic";

// File des signalements (F6.1).

const API = process.env.API_INTERNAL_URL ?? "http://localhost:8080";
const TOKEN = process.env.ADMIN_TOKEN ?? "";

type Report = {
  id: string;
  reporter_pseudo: string;
  target_type: string;
  target_id: string;
  reason: string;
  comment: string | null;
  status: string;
  outcome: string | null;
  created_at: string;
};

async function closeReport(formData: FormData) {
  "use server";
  await fetch(`${API}/admin/reports/${formData.get("id")}/close`, {
    method: "POST",
    headers: { "Content-Type": "application/json", "X-Admin-Token": TOKEN },
    body: JSON.stringify({ outcome: formData.get("outcome") }),
  });
  revalidatePath("/admin/signalements");
}

export default async function AdminSignalementsPage() {
  const response = await fetch(`${API}/admin/reports`, {
    headers: { "X-Admin-Token": TOKEN },
    cache: "no-store",
  });
  const reports: Report[] = response.ok ? await response.json() : [];

  return (
    <main className="mx-auto flex w-full max-w-3xl flex-col gap-3 p-6 font-mono text-sm">
      <h1 className="text-xl font-bold">
        Signalements ({reports.filter((r) => r.status === "nouveau").length} nouveaux)
      </h1>
      <a href="/admin" className="text-xs underline">
        ← Admin
      </a>
      {reports.map((r) => (
        <div key={r.id} className="rounded border p-3">
          <p>
            [{r.status}
            {r.outcome ? ` → ${r.outcome}` : ""}] {r.target_type} · {r.reason} · par{" "}
            {r.reporter_pseudo} · {new Date(r.created_at).toLocaleDateString("fr-FR")}
          </p>
          <p>cible : {r.target_id}</p>
          {r.comment ? <p>« {r.comment} »</p> : null}
          {r.status === "nouveau" ? (
            <div className="mt-2 flex gap-2">
              <form action={closeReport}>
                <input type="hidden" name="id" value={r.id} />
                <input type="hidden" name="outcome" value="fonde" />
                <button type="submit" className="border bg-black px-3 py-1 text-white">
                  Fondé (+2 au score)
                </button>
              </form>
              <form action={closeReport}>
                <input type="hidden" name="id" value={r.id} />
                <input type="hidden" name="outcome" value="rejete" />
                <button type="submit" className="border px-3 py-1">
                  Rejeter
                </button>
              </form>
            </div>
          ) : null}
        </div>
      ))}
      {reports.length === 0 ? <p>Aucun signalement. Tout va bien.</p> : null}
    </main>
  );
}
