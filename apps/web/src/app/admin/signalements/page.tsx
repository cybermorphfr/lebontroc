import { revalidatePath } from "next/cache";
import { adminFetch, adminPost } from "../adminFetch";

export const dynamic = "force-dynamic";

// File des signalements (F6.1).


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
  await adminPost(`/admin/reports/${formData.get("id")}/close`, { outcome: formData.get("outcome") });
  revalidatePath("/admin/signalements");
}

export default async function AdminSignalementsPage() {
  const reports: Report[] = (await adminFetch<Report[]>(`/admin/reports`)) ?? [];

  return (
    <main className="flex flex-col gap-3 text-sm">
      <h1 className="font-display text-2xl">
        Signalements ({reports.filter((r) => r.status === "nouveau").length} nouveaux)
      </h1>
      {reports.map((r) => (
        <div key={r.id} className="flex flex-col gap-1 rounded-[28px] bg-sable p-5 shadow-sm">
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
                <button type="submit" className="cursor-pointer rounded-full bg-[#c67139] px-4 py-1.5 font-display text-xs text-creme hover:bg-terracotta-600">
                  Fondé (+2 au score)
                </button>
              </form>
              <form action={closeReport}>
                <input type="hidden" name="id" value={r.id} />
                <input type="hidden" name="outcome" value="rejete" />
                <button type="submit" className="cursor-pointer rounded-full border border-neutre-300 px-4 py-1.5 text-xs hover:bg-encre/7">
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
