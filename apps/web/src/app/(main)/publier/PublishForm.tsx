"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useRef, useState } from "react";
import type { CategoryNode, ItemResponse } from "@lebontroc/api-client";

import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { Segmented } from "@/components/ui/Segmented";
import { Textarea } from "@/components/ui/Textarea";
import { apiError, apiFetch } from "@/lib/client-api";

import { CategoryPicker, findCategory } from "./CategoryPicker";
import { PhotoGrid, type PhotoState } from "./PhotoGrid";

const CONDITIONS = [
  { value: "neuf", label: "Neuf" },
  { value: "tres_bon_etat", label: "Très bon état" },
  { value: "bon_etat", label: "Bon état" },
  { value: "correct", label: "Correct" },
];

const REMISES = [
  { value: "main_propre", label: "Main propre" },
  { value: "envoi", label: "Envoi" },
  { value: "les_deux", label: "Les deux" },
];

type FieldErrors = Partial<Record<"title" | "category" | "description" | "value", string>>;

export function PublishForm({
  categories,
  verified,
  editItem,
}: {
  categories: CategoryNode[];
  verified: boolean;
  editItem?: ItemResponse;
}) {
  const router = useRouter();
  const [photos, setPhotos] = useState<PhotoState[]>(
    editItem?.photos.map((p) => ({
      localId: p.photo_id,
      photoId: p.photo_id,
      previewUrl: p.url,
      uploading: false,
      failed: false,
    })) ?? [],
  );
  const [title, setTitle] = useState(editItem?.title ?? "");
  const [categoryId, setCategoryId] = useState<number | null>(editItem?.category_id ?? null);
  const [condition, setCondition] = useState(editItem?.condition ?? "tres_bon_etat");
  const [description, setDescription] = useState(editItem?.description ?? "");
  const [value, setValue] = useState(
    editItem ? String(Math.round(editItem.value_cents / 100)) : "",
  );
  const [deliveryPref, setDeliveryPref] = useState(editItem?.delivery_pref ?? "main_propre");
  const [wishes, setWishes] = useState(editItem?.exchange_wishes ?? "");
  const [acceptsSoulte, setAcceptsSoulte] = useState(editItem?.accepts_soulte ?? true);
  const [errors, setErrors] = useState<FieldErrors>({});
  const [photoError, setPhotoError] = useState<string | null>(null);
  const [globalError, setGlobalError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [published, setPublished] = useState<string | null>(null);

  const draftId = useRef(crypto.randomUUID());
  const startedAt = useRef<number | null>(null);

  function trackStart() {
    if (startedAt.current !== null) return;
    startedAt.current = Date.now();
    if (editItem) return;
    fetch("/api/analytics/track", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "item_publish_started" }),
    }).catch(() => {});
  }

  const selectedCategory = findCategory(categories, categoryId);
  const valueNumber = Number.parseInt(value, 10);
  const rangeMin = selectedCategory?.value_min_cents ?? null;
  const rangeMax = selectedCategory?.value_max_cents ?? null;
  const overRange =
    rangeMax !== null && Number.isFinite(valueNumber) && valueNumber * 100 > rangeMax * 3;

  const valueHint =
    rangeMin !== null && rangeMax !== null
      ? `Les objets de cette catégorie tournent entre ${Math.round(rangeMin / 100)} et ${Math.round(rangeMax / 100)} €.`
      : "Ce n'est pas un prix — juste un repère pour équilibrer les trocs.";

  const uploading = photos.some((p) => p.uploading);
  const failedCount = photos.filter((p) => p.failed).length;
  const buttonLabel = !verified
    ? "Vérifie ton e-mail pour publier"
    : photos.length === 0
      ? "Ajoute au moins une photo"
      : uploading
        ? "Envoi des photos…"
        : failedCount > 0
          ? "Une photo n'a pas pu être envoyée"
          : submitting
            ? editItem
              ? "Enregistrement…"
              : "Publication…"
            : editItem
              ? "Enregistrer"
              : "Publier";
  const buttonDisabled =
    !verified || photos.length === 0 || uploading || failedCount > 0 || submitting;

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    const next: FieldErrors = {};
    if (title.trim().length < 3) next.title = "Donne un titre à ton objet.";
    if (title.trim().length > 80) next.title = "80 caractères maximum.";
    if (categoryId === null) next.category = "Choisis une catégorie pour que ton objet soit trouvable.";
    if (description.trim().length < 10)
      next.description = "Décris ton objet en quelques mots (10 caractères minimum).";
    if (!Number.isFinite(valueNumber) || valueNumber < 1 || valueNumber > 2000)
      next.value = "Indique une valeur, même approximative — elle guide les propositions.";
    setErrors(next);
    if (Object.keys(next).length > 0) return;

    const photoIds = photos.map((p) => p.photoId).filter((id): id is string => id !== null);
    if (photoIds.length === 0) {
      setPhotoError("L'envoi a échoué. Touche la photo pour réessayer.");
      return;
    }
    setSubmitting(true);
    setGlobalError(null);
    const payload = {
      title: title.trim(),
      description: description.trim(),
      category_id: categoryId,
      condition,
      value_cents: valueNumber * 100,
      delivery_pref: deliveryPref,
      exchange_wishes: wishes.trim() || null,
      accepts_soulte: acceptsSoulte,
    };

    let response: Response;
    if (editItem) {
      response = await apiFetch(`/items/${editItem.id}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ ...payload, status: editItem.status }),
      });
      if (response.ok) {
        const samePhotos =
          photoIds.length === editItem.photos.length &&
          photoIds.every((id, index) => editItem.photos[index]?.photo_id === id);
        if (!samePhotos) {
          response = await apiFetch(`/items/${editItem.id}/photos`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ photos: photoIds }),
          });
        }
      }
      if (response.ok) {
        router.push("/dressing");
        router.refresh();
        return;
      }
    } else {
      response = await apiFetch("/items", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          ...payload,
          photos: photoIds,
          draft_id: draftId.current,
          duration_seconds:
            startedAt.current !== null
              ? Math.round((Date.now() - startedAt.current) / 1000)
              : null,
        }),
      });
      if (response.ok) {
        const item = (await response.json()) as ItemResponse;
        setPublished(item.title);
        return;
      }
    }
    setSubmitting(false);
    setGlobalError((await apiError(response)).message);
  }

  if (published !== null) {
    return (
      <section className="flex flex-col items-start gap-4 rounded-[32px] bg-sable p-6 shadow-sm">
        <div
          aria-hidden
          className="flex size-14 items-center justify-center rounded-full bg-sauge-100 text-sauge-700"
        >
          <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.75" strokeLinecap="round" strokeLinejoin="round">
            <path d="M20 6 9 17l-5-5" />
          </svg>
        </div>
        <h1 className="font-display text-3xl">C&apos;est publié !</h1>
        <p className="text-neutre-700">
          <strong className="text-encre">{published}</strong> est maintenant visible par les
          troqueurs autour de toi.
        </p>
        <div className="flex flex-wrap gap-2">
          <Link
            href="/dressing"
            className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-7 py-3 font-display text-base text-creme transition-colors hover:bg-terracotta-600"
          >
            Voir mon dressing
          </Link>
          <Button variant="ghost" size="lg" onClick={() => window.location.reload()}>
            Publier un autre objet
          </Button>
        </div>
      </section>
    );
  }

  return (
    <form
      onSubmit={submit}
      onFocus={trackStart}
      className="flex flex-col gap-5 rounded-[32px] bg-sable p-6 shadow-sm"
      noValidate
    >
      <h1 className="font-display text-3xl">
        {editItem ? "Modifie ton objet" : "Publie ton objet"}
      </h1>

      <PhotoGrid
        photos={photos}
        setPhotos={setPhotos}
        onFirstInteraction={trackStart}
        onError={setPhotoError}
      />
      {photoError ? <p className="-mt-3 text-[11px] text-terracotta-700">{photoError}</p> : null}

      <Input
        id="title"
        label="Titre"
        placeholder="Ex : Poussette Yoyo"
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        error={errors.title}
      />

      <CategoryPicker
        categories={categories}
        value={categoryId}
        onChange={(id) => setCategoryId(id)}
        error={errors.category}
      />

      <Segmented label="État" options={CONDITIONS} value={condition} onChange={setCondition} />

      <Textarea
        id="description"
        label="Description"
        placeholder="Marque, taille, petits défauts… tout ce que tu voudrais savoir avant de troquer."
        value={description}
        onChange={(e) => setDescription(e.target.value)}
        error={errors.description}
      />

      <Input
        id="value"
        label="Valeur indicative (€)"
        placeholder="Ex : 150"
        inputMode="numeric"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        error={errors.value}
        hint={
          overRange
            ? "C'est bien au-dessus des objets similaires — vérifie, ça aide à recevoir des propositions justes."
            : valueHint
        }
        className={overRange ? "border-terracotta-500" : ""}
      />

      <Segmented label="Remise" options={REMISES} value={deliveryPref} onChange={setDeliveryPref} />

      <Input
        id="wishes"
        label="Ce que j'aimerais en échange (optionnel)"
        placeholder="Vélo enfant, jeux de société…"
        hint="Ça aide les autres à te proposer le bon troc."
        value={wishes}
        onChange={(e) => setWishes(e.target.value)}
        maxLength={300}
      />

      <label className="flex cursor-pointer items-center justify-between gap-3 rounded-3xl bg-sable p-4">
        <span className="flex flex-col">
          <span className="text-sm font-semibold">J&apos;accepte une soulte</span>
          <span className="text-xs text-neutre-700">
            Un complément en euros si les valeurs ne collent pas tout à fait.
          </span>
        </span>
        <input
          type="checkbox"
          checked={acceptsSoulte}
          onChange={(e) => setAcceptsSoulte(e.target.checked)}
          className="size-5 accent-[#c67139]"
        />
      </label>

      {globalError ? (
        <p className="rounded-full bg-terracotta-100 px-4 py-2 text-sm text-terracotta-800">
          {globalError}
        </p>
      ) : null}

      <Button type="submit" size="lg" block disabled={buttonDisabled}>
        {buttonLabel}
      </Button>
    </form>
  );
}
