"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";
import type { ItemResponse } from "@lebontroc/api-client";

import { BottomSheet } from "@/components/ui/BottomSheet";
import { Tag } from "@/components/ui/Tag";
import { apiFetch } from "@/lib/client-api";

const STATUS_LABELS: Record<string, { label: string; variant: "accent" | "accent-2" | "neutral" }> = {
  disponible: { label: "Disponible", variant: "accent-2" },
  reserve: { label: "Réservé", variant: "accent" },
  troque: { label: "Troqué", variant: "neutral" },
  masque: { label: "Masqué", variant: "neutral" },
};

export function DressingGrid({ items }: { items: ItemResponse[] }) {
  const router = useRouter();
  const [selected, setSelected] = useState<ItemResponse | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  async function toggleHidden(item: ItemResponse) {
    await apiFetch(`/items/${item.id}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        title: item.title,
        description: item.description,
        category_id: item.category_id,
        condition: item.condition,
        value_cents: item.value_cents,
        delivery_pref: item.delivery_pref,
        exchange_wishes: item.exchange_wishes,
        status: item.status === "masque" ? "disponible" : "masque",
      }),
    });
    setSelected(null);
    router.refresh();
  }

  async function deleteItem(item: ItemResponse) {
    const response = await apiFetch(`/items/${item.id}`, { method: "DELETE" });
    setConfirmDelete(false);
    setSelected(null);
    if (response.ok) {
      setToast("Objet supprimé.");
      setTimeout(() => setToast(null), 4000);
    }
    router.refresh();
  }

  function closeSheet() {
    setSelected(null);
    setConfirmDelete(false);
  }

  const toastElement = toast ? (
    <p className="fixed bottom-4 left-1/2 z-50 -translate-x-1/2 rounded-full bg-encre px-5 py-2 text-sm text-creme shadow-lg">
      {toast}
    </p>
  ) : null;

  if (items.length === 0) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-[32px] bg-sable p-6 shadow-sm">
        <h2 className="font-display text-xl">Ton dressing est vide</h2>
        <p className="text-sm text-neutre-700">
          Publie ton premier objet — c&apos;est lui qui te permettra de troquer.
        </p>
        <Link
          href="/publier"
          className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-6 py-2.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
        >
          Publier un objet
        </Link>
        {toastElement}
      </div>
    );
  }

  return (
    <>
      <ul className="grid grid-cols-2 gap-3.5 sm:grid-cols-3">
        {items.map((item) => {
          const status = STATUS_LABELS[item.status] ?? STATUS_LABELS.disponible;
          const cover = item.photos[0]?.url;
          return (
            <li
              key={item.id}
              className={`flex flex-col overflow-hidden rounded-3xl bg-sable shadow-sm ${
                item.status === "masque" ? "opacity-70" : ""
              }`}
            >
              <div className="relative aspect-square bg-neutre-100">
                {cover ? (
                  // eslint-disable-next-line @next/next/no-img-element
                  <img src={cover} alt={item.title} className="size-full object-cover" />
                ) : null}
                <button
                  aria-label={`Actions pour ${item.title}`}
                  onClick={() => setSelected(item)}
                  className="absolute right-2 top-2 flex size-7 items-center justify-center rounded-full bg-creme/85 text-encre"
                >
                  ⋯
                </button>
              </div>
              <div className="flex flex-col gap-1 p-3">
                <span className="truncate text-sm font-semibold">{item.title}</span>
                <div className="flex items-center justify-between gap-2">
                  <Tag variant={status.variant}>{status.label}</Tag>
                  <span className="text-xs text-neutre-700">
                    {Math.round(item.value_cents / 100)} €
                  </span>
                </div>
              </div>
            </li>
          );
        })}
      </ul>
      <p className="mt-3 text-[11px] text-neutre-700">
        Les objets masqués ne sont visibles que par toi.
      </p>

      {toastElement}

      <BottomSheet
        open={selected !== null}
        onClose={closeSheet}
        title={confirmDelete && selected ? `Supprimer ${selected.title} ?` : selected?.title}
      >
        {selected && confirmDelete ? (
          <div className="flex flex-col gap-3">
            <p className="text-sm text-neutre-700">
              L&apos;objet et ses photos seront supprimés pour de bon. Si tu veux juste faire une
              pause, masque-le plutôt.
            </p>
            <button
              onClick={() => deleteItem(selected)}
              className="flex min-h-11 cursor-pointer items-center justify-center rounded-full bg-terracotta-800 px-5 font-display text-sm text-creme"
            >
              Supprimer pour de bon
            </button>
            {selected.status === "disponible" ? (
              <button
                onClick={() => toggleHidden(selected)}
                className="flex min-h-11 cursor-pointer items-center justify-center rounded-full px-5 text-sm text-terracotta-700 hover:bg-terracotta-500/10"
              >
                Masquer plutôt
              </button>
            ) : null}
            <button
              onClick={() => setConfirmDelete(false)}
              className="flex min-h-11 cursor-pointer items-center justify-center rounded-full px-5 text-sm text-neutre-700 hover:bg-encre/5"
            >
              Annuler
            </button>
          </div>
        ) : selected ? (
          <div className="flex flex-col gap-1">
            <Link
              href={`/publier?objet=${selected.id}`}
              className="flex min-h-12 items-center border-b border-neutre-300/60 px-1 text-sm hover:bg-encre/5"
            >
              Modifier
            </Link>
            {selected.status === "disponible" || selected.status === "masque" ? (
              <button
                onClick={() => toggleHidden(selected)}
                className="flex min-h-12 cursor-pointer items-center border-b border-neutre-300/60 px-1 text-left text-sm hover:bg-encre/5"
              >
                {selected.status === "masque" ? "Remettre en ligne" : "Masquer"}
              </button>
            ) : null}
            {selected.status === "disponible" || selected.status === "masque" ? (
              <button
                onClick={() => setConfirmDelete(true)}
                className="flex min-h-12 cursor-pointer items-center px-1 text-left text-sm text-terracotta-800 hover:bg-terracotta-500/10"
              >
                Supprimer
              </button>
            ) : null}
          </div>
        ) : null}
      </BottomSheet>
    </>
  );
}
