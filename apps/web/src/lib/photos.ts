/**
 * Compression photo côté client : redimensionnement à 1600 px max côté long,
 * WebP q0.82 (fallback JPEG q0.85 — Safari ne sait pas encoder WebP).
 * Totalement transparente pour l'utilisateur.
 */
export async function compressImage(
  file: File,
): Promise<{ blob: Blob; contentType: "image/webp" | "image/jpeg" }> {
  const bitmap = await createImageBitmap(file).catch(() => null);
  if (!bitmap) throw new Error("image illisible");

  const maxSide = 1600;
  const scale = Math.min(1, maxSide / Math.max(bitmap.width, bitmap.height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(bitmap.width * scale);
  canvas.height = Math.round(bitmap.height * scale);
  const context = canvas.getContext("2d");
  if (!context) throw new Error("canvas indisponible");
  context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  bitmap.close();

  const toBlob = (type: string, quality: number) =>
    new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, type, quality));

  const webp = await toBlob("image/webp", 0.82);
  if (webp && webp.type === "image/webp") {
    return { blob: webp, contentType: "image/webp" };
  }
  const jpeg = await toBlob("image/jpeg", 0.85);
  if (jpeg && jpeg.type === "image/jpeg") {
    return { blob: jpeg, contentType: "image/jpeg" };
  }
  throw new Error("encodage impossible");
}
