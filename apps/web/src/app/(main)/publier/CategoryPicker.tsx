"use client";

import { useState } from "react";
import type { CategoryNode } from "@lebontroc/api-client";

import { BottomSheet } from "@/components/ui/BottomSheet";

/** Chemin d'affichage d'une catégorie (« Enfants › Poussettes et portage »). */
export function categoryPath(categories: CategoryNode[], id: number | null): string | null {
  if (id === null) return null;
  for (const root of categories) {
    if (root.id === id) return root.label;
    for (const child of root.children) {
      if (child.id === id) return `${root.label} › ${child.label}`;
      for (const leaf of child.children) {
        if (leaf.id === id) return `${root.label} › ${child.label} › ${leaf.label}`;
      }
    }
  }
  return null;
}

export function findCategory(categories: CategoryNode[], id: number | null): CategoryNode | null {
  if (id === null) return null;
  const stack = [...categories];
  while (stack.length > 0) {
    const node = stack.pop();
    if (!node) break;
    if (node.id === id) return node;
    stack.push(...node.children);
  }
  return null;
}

export function CategoryPicker({
  categories,
  value,
  onChange,
  error,
}: {
  categories: CategoryNode[];
  value: number | null;
  onChange: (id: number) => void;
  error?: string;
}) {
  const [open, setOpen] = useState(false);
  const [current, setCurrent] = useState<CategoryNode | null>(null);

  const path = categoryPath(categories, value);
  const list = current ? current.children : categories;

  function pick(node: CategoryNode) {
    if (node.children.length > 0) {
      setCurrent(node);
    } else {
      onChange(node.id);
      setOpen(false);
      setCurrent(null);
    }
  }

  return (
    <div className="flex flex-col">
      <span className="mb-1.5 text-xs text-encre/70">Catégorie</span>
      <button
        type="button"
        onClick={() => setOpen(true)}
        aria-haspopup="dialog"
        className={`flex min-h-9 w-full items-center justify-between rounded-full border bg-sable px-4 py-1.5 text-left text-sm transition-colors hover:border-encre/45 ${
          error ? "border-terracotta-500" : "border-neutre-300"
        } ${path ? "text-encre" : "text-neutre-500"}`}
      >
        <span className="truncate">{path ?? "Choisis une catégorie"}</span>
        <ChevronDown />
      </button>
      {error ? <p className="mt-1 text-[11px] text-terracotta-700">{error}</p> : null}

      <BottomSheet
        open={open}
        onClose={() => {
          setOpen(false);
          setCurrent(null);
        }}
        title={current ? current.label : "Catégorie"}
      >
        {current ? (
          <button
            type="button"
            onClick={() => setCurrent(null)}
            className="mb-2 flex items-center gap-1 text-sm text-terracotta-700"
          >
            ← Toutes les catégories
          </button>
        ) : null}
        <ul className="flex flex-col">
          {list.map((node) => (
            <li key={node.id}>
              <button
                type="button"
                onClick={() => pick(node)}
                className="flex min-h-12 w-full items-center justify-between border-b border-neutre-300/60 px-1 text-left text-sm transition-colors hover:bg-encre/5"
              >
                {node.label}
                {node.children.length > 0 ? <ChevronRight /> : null}
              </button>
            </li>
          ))}
        </ul>
      </BottomSheet>
    </div>
  );
}

function ChevronDown() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="m6 9 6 6 6-6" />
    </svg>
  );
}

function ChevronRight() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="m9 18 6-6-6-6" />
    </svg>
  );
}
