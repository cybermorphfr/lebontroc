"use client";

import { apiFetch } from "@/lib/client-api";
import { compressImage } from "@/lib/photos";

export type PhotoState = {
  /** Clé locale stable pour React. */
  localId: string;
  /** Id serveur (présent une fois l'upload réussi). */
  photoId: string | null;
  /** Aperçu local ou URL publique (mode édition). */
  previewUrl: string;
  uploading: boolean;
  failed: boolean;
  /** Blob compressé, conservé pour rejouer l'upload en cas d'échec. */
  blob?: Blob;
  contentType?: string;
};

export const PHOTOS_MAX = 8;

async function uploadOne(
  photo: PhotoState,
  update: (localId: string, patch: Partial<PhotoState>) => void,
) {
  if (!photo.blob || !photo.contentType) return;
  update(photo.localId, { uploading: true, failed: false });
  try {
    const presign = await apiFetch("/items/photos/presign", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        files: [{ content_type: photo.contentType, size: photo.blob.size }],
      }),
    });
    if (!presign.ok) throw new Error("presign");
    const [{ photo_id, upload_url }] = (await presign.json()) as Array<{
      photo_id: string;
      upload_url: string;
    }>;
    const put = await fetch(upload_url, {
      method: "PUT",
      headers: { "Content-Type": photo.contentType },
      body: photo.blob,
    });
    if (!put.ok) throw new Error("put");
    update(photo.localId, { photoId: photo_id, uploading: false });
  } catch {
    update(photo.localId, { uploading: false, failed: true });
  }
}

export function PhotoGrid({
  photos,
  setPhotos,
  onFirstInteraction,
  onError,
}: {
  photos: PhotoState[];
  setPhotos: React.Dispatch<React.SetStateAction<PhotoState[]>>;
  onFirstInteraction: () => void;
  onError: (message: string | null) => void;
}) {
  function update(localId: string, patch: Partial<PhotoState>) {
    setPhotos((current) =>
      current.map((p) => (p.localId === localId ? { ...p, ...patch } : p)),
    );
  }

  async function addFiles(list: FileList | null) {
    onFirstInteraction();
    onError(null);
    if (!list) return;
    const remaining = PHOTOS_MAX - photos.length;
    if (list.length > remaining) {
      onError("8 photos maximum — garde les meilleures !");
    }
    for (const file of Array.from(list).slice(0, Math.max(0, remaining))) {
      let compressed;
      try {
        compressed = await compressImage(file);
      } catch {
        onError("On n'a pas réussi à lire cette image. Essaie un autre format (JPG, PNG, WebP).");
        continue;
      }
      if (compressed.blob.size > 5 * 1024 * 1024) {
        onError("Cette photo dépasse 5 Mo après compression. Réessaie avec une autre.");
        continue;
      }
      const photo: PhotoState = {
        localId: crypto.randomUUID(),
        photoId: null,
        previewUrl: URL.createObjectURL(compressed.blob),
        uploading: false,
        failed: false,
        blob: compressed.blob,
        contentType: compressed.contentType,
      };
      setPhotos((current) => [...current, photo]);
      void uploadOne(photo, update);
    }
  }

  function move(index: number, delta: number) {
    setPhotos((current) => {
      const target = index + delta;
      if (target < 0 || target >= current.length) return current;
      const next = [...current];
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
  }

  function remove(localId: string) {
    setPhotos((current) => current.filter((p) => p.localId !== localId));
  }

  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-xs text-encre/70">Photos</span>
      <div className="flex gap-2 overflow-x-auto pb-1">
        {photos.map((photo, index) => (
          <div
            key={photo.localId}
            className="relative size-24 flex-none overflow-hidden rounded-2xl bg-sable"
          >
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img
              src={photo.previewUrl}
              alt={`Photo ${index + 1}`}
              className="size-full object-cover"
            />
            {index === 0 ? (
              <span className="absolute left-1 top-1 rounded-full bg-encre/70 px-2 py-0.5 text-[10px] text-creme">
                Couverture
              </span>
            ) : null}
            {photo.uploading ? (
              <span className="absolute inset-0 flex items-center justify-center bg-encre/40">
                <span className="size-5 animate-spin rounded-full border-2 border-creme border-t-transparent" />
              </span>
            ) : null}
            {photo.failed ? (
              <button
                type="button"
                onClick={() => uploadOne(photo, update)}
                aria-label="Réessayer l'envoi de la photo"
                className="absolute inset-0 flex items-center justify-center border-2 border-terracotta-500 bg-terracotta-100/70 text-terracotta-700"
              >
                <RetryIcon />
              </button>
            ) : null}
            <button
              type="button"
              aria-label="Supprimer la photo"
              onClick={() => remove(photo.localId)}
              className="absolute right-1 top-1 flex size-6 items-center justify-center rounded-full bg-creme/85 text-encre"
            >
              ✕
            </button>
            <div className="absolute bottom-1 left-1 right-1 flex justify-between">
              <button
                type="button"
                aria-label="Déplacer la photo vers la gauche"
                onClick={() => move(index, -1)}
                disabled={index === 0}
                className="flex size-6 items-center justify-center rounded-full bg-creme/85 text-xs disabled:opacity-40"
              >
                ←
              </button>
              <button
                type="button"
                aria-label="Déplacer la photo vers la droite"
                onClick={() => move(index, 1)}
                disabled={index === photos.length - 1}
                className="flex size-6 items-center justify-center rounded-full bg-creme/85 text-xs disabled:opacity-40"
              >
                →
              </button>
            </div>
          </div>
        ))}
        {photos.length < PHOTOS_MAX ? (
          <label
            className={`flex flex-none cursor-pointer items-center justify-center rounded-2xl border-[1.5px] border-dashed border-terracotta-500 bg-terracotta-100 text-terracotta-700 transition-colors hover:bg-terracotta-100/70 ${
              photos.length === 0 ? "size-24" : "size-24"
            }`}
          >
            <span className="sr-only">Ajouter une photo</span>
            <CameraIcon />
            <input
              type="file"
              accept="image/*"
              multiple
              className="hidden"
              onChange={(e) => {
                void addFiles(e.target.files);
                e.target.value = "";
              }}
            />
          </label>
        ) : null}
      </div>
      <p className="text-[11px] text-neutre-700">1 à 8 photos — la première fait la vignette.</p>
    </div>
  );
}

function CameraIcon() {
  return (
    <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z" />
      <circle cx="12" cy="13" r="3" />
    </svg>
  );
}

function RetryIcon() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
      <path d="M3 3v5h5" />
    </svg>
  );
}
