import { expect, test, type Page } from "@playwright/test";

// F1.1 — Scénarios Gherkin :
//   « publication complète » : un utilisateur vérifié publie un objet avec
//   photos, il apparaît dans son dressing en « disponible », la première
//   photo est la vignette.
//   « publication impossible sans photo » : bouton désactivé avec message.

const MAILPIT = process.env.MAILPIT_URL ?? "http://localhost:8025";

// PNG 1×1 — recompressé en WebP/JPEG par le client avant upload.
const TINY_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

async function verifiedUser(page: Page): Promise<string> {
  const email = `publieur-${Date.now()}-${Math.floor(Math.random() * 10000)}@exemple.fr`;
  await page.goto("/inscription");
  await page.getByLabel("Pseudo").fill(`pub${Date.now() % 100000}`);
  await page.getByLabel("E-mail").fill(email);
  await page.getByLabel("Mot de passe", { exact: true }).fill("un-bon-mot-de-passe");
  await page.getByLabel("Code postal").fill("44000");
  await page.getByRole("button", { name: "Créer mon compte" }).click();
  await expect(page).toHaveURL(/\/verification$/);

  let messageId = "";
  await expect
    .poll(async () => {
      const response = await fetch(
        `${MAILPIT}/api/v1/search?query=${encodeURIComponent(`to:${email}`)}`,
      );
      const body = (await response.json()) as { messages?: Array<{ ID: string }> };
      messageId = body.messages?.[0]?.ID ?? "";
      return messageId;
    }, { timeout: 15000 })
    .not.toBe("");
  const message = (await (await fetch(`${MAILPIT}/api/v1/message/${messageId}`)).json()) as {
    Text: string;
  };
  const link = message.Text.match(/https?:\/\/[^\s]+verify-email\?token=[A-Za-z0-9_-]+/)?.[0];
  if (!link) throw new Error("lien de vérification introuvable");
  await page.goto(link);
  await expect(page).toHaveURL(/statut=ok/);
  return email;
}

test("publication complète : photos, formulaire, dressing", async ({ page }) => {
  await verifiedUser(page);

  await page.goto("/publier");
  await expect(page.getByRole("heading", { name: "Publie ton objet" })).toBeVisible();

  // Sans photo : le bouton porte le message et reste désactivé (Gherkin).
  const disabledButton = page.getByRole("button", { name: "Ajoute au moins une photo" });
  await expect(disabledButton).toBeVisible();
  await expect(disabledButton).toBeDisabled();

  // Deux photos.
  const input = page.locator('input[type="file"]');
  await input.setInputFiles([
    { name: "photo1.png", mimeType: "image/png", buffer: TINY_PNG },
    { name: "photo2.png", mimeType: "image/png", buffer: TINY_PNG },
  ]);
  await expect(page.getByText("Couverture")).toBeVisible();

  await page.getByLabel("Titre").fill("Poussette Yoyo");
  await page.getByRole("button", { name: "Choisis une catégorie" }).click();
  await page.getByRole("button", { name: "Enfants et puériculture" }).click();
  await page.getByRole("button", { name: "Poussettes et portage" }).click();
  await page
    .getByLabel("Description")
    .fill("Très bon état, pliage une main, avec l'ombrelle d'origine.");
  await page.getByLabel("Valeur indicative (€)").fill("150");
  // La fourchette de la catégorie guide la valeur.
  await expect(page.getByText(/tournent entre 2 et 250 €/)).toBeVisible();

  // Publier (attend la fin des uploads automatiquement : bouton réactivé).
  await page.getByRole("button", { name: "Publier", exact: true }).click();
  await expect(page.getByRole("heading", { name: "C'est publié !" })).toBeVisible();

  // Le dressing montre l'objet en « disponible » avec sa vignette.
  await page.getByRole("link", { name: "Voir mon dressing" }).click();
  await expect(page).toHaveURL(/\/dressing/);
  await expect(page.getByText("Poussette Yoyo")).toBeVisible();
  await expect(page.getByText("Disponible")).toBeVisible();
  const vignette = page.locator("img[alt='Poussette Yoyo']");
  await expect(vignette).toBeVisible();
});

test("un compte non vérifié ne peut pas publier", async ({ page }) => {
  // Inscription SANS clic sur le lien de vérification.
  await page.goto("/inscription");
  await page.getByLabel("Pseudo").fill(`nover${Date.now() % 100000}`);
  await page.getByLabel("E-mail").fill(`nover-${Date.now()}@exemple.fr`);
  await page.getByLabel("Mot de passe", { exact: true }).fill("un-bon-mot-de-passe");
  await page.getByLabel("Code postal").fill("44000");
  await page.getByRole("button", { name: "Créer mon compte" }).click();
  await expect(page).toHaveURL(/\/verification$/);

  await page.goto("/publier");
  const button = page.getByRole("button", { name: "Vérifie ton e-mail pour publier" });
  await expect(button).toBeVisible();
  await expect(button).toBeDisabled();
});
